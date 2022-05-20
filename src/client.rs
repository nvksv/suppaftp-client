use suppaftp::{sync_ftp::FtpStream, types::{FtpResult, FtpError}, list};
use crate::{
    mlst::{MlstFact, parse_mlst_feat, parse_mlst_line, list_to_ftp},
    types::{FtpItem, FtpItemType, FtpList}
};
use native_tls::{TlsConnector};
use std::str::FromStr;

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FtpClientFeatures {
    clnt: bool,
    pasv: bool,
    utf8: bool,
    mdtm: bool,
    size: bool,
    rest_stream: bool,
    tvfs: bool,
    mlst: Option<Vec<(MlstFact, bool)>>,
    auth_tls: bool,
    others: Vec<String>,
}

impl Default for FtpClientFeatures {
    fn default() -> Self {
        Self {
            clnt: false,
            pasv: false,
            utf8: false,
            mdtm: false,
            size: false,
            rest_stream: false,
            tvfs: false,
            mlst: None,
            auth_tls: false,
            others: vec![],
        }
    }
}

impl From<Vec<String>> for FtpClientFeatures {

    fn from(lines: Vec<String>) -> Self {
        let mut result = Self::default();

        for line in lines {
            let trimmed_line = line.trim();

            if trimmed_line.is_empty() || trimmed_line.chars().all(char::is_whitespace) {
                continue;
            }

            let (first_word, tail) = match trimmed_line.split_once(|ch| ch == ' ') {
                Some((fw, t)) => (fw.trim(), t.trim()),
                None => (trimmed_line, "")
            };

            match first_word {
                "CLNT" => { 
                    result.clnt = true; 
                },
                "PASV" => { 
                    result.pasv = true; 
                },
                "UTF8" => { 
                    result.utf8 = true; 
                },
                "MDTM" => { 
                    result.mdtm = true; 
                },
                "SIZE" => { 
                    result.size = true; 
                },
                "REST" if tail.eq_ignore_ascii_case("STREAM") => { 
                    result.rest_stream = true;
                },
                "TVFS" => { 
                    result.tvfs = true; 
                },
                "MLST" => { 
                    result.mlst = Some(parse_mlst_feat(tail)); 
                },
                "AUTH" if tail.eq_ignore_ascii_case("TLS") => { 
                    result.auth_tls = true;
                },
                _ => {
                    result.others.push( line );
                }
            }

        };

        result
    }
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FtpClientListMode {
    List,
    Nlst,
    Mlsd,
    Stat,
}

pub enum FtpClientPathMode {
    Linux,
    Windows,
    StepByStep,
}

pub trait FtpClientSettings: std::fmt::Debug {
    fn addr(&self) -> &str;
    fn login(&self) -> &str;
    fn password(&self) -> &str;
    fn remote_dir(&self) -> Option<&str>;
    
    #[cfg(feature = "secure")]
    fn use_secure(&self) -> bool {
        true
    }

    #[cfg(feature = "secure")]
    fn sni(&self) -> Option<&str>;

    fn use_feat(&self) -> bool {
        true
    }

    fn use_passive_mode(&self) -> bool {
        true
    }

    fn list_mode(&self) -> Option<FtpClientListMode> {
        None
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum FtpPath {
    Windows(String),
    Linux(String),
    StepByStep(Vec<String>),
}

#[derive(Debug)]
pub struct FtpClient {
    settings: Box<dyn FtpClientSettings>,
    settings_list_mode: Option<FtpClientListMode>,
    effective_list_mode: Option<FtpClientListMode>,

    ftp: Option<FtpStream>,

    has_feat: bool,
    features: FtpClientFeatures,

    current_path: Option<FtpPath>,
}

macro_rules! ftp {
    ($self:expr, $func:ident($($params:tt)*)) => {{
        let mut already_reconnected = false;
        
        let mut ftp = match $self.ftp.as_mut() {
            Some(ftp) => ftp,
            None => {
                already_reconnected = true;
                $self.reconnect()?
            }
        };

        let mut result = ftp.$func($($params)*);

        if let Err(e) = &result {
            if e.is_recoverable() && !already_reconnected {
                ftp = $self.reconnect()?;
                result = ftp.$func($($params)*);
            };
        };

        result
    }};
}

macro_rules! list_fn {
    ($self: expr, $func: ident, $map: expr) => {
        ftp!($self, $func(None))?
            .into_iter()
            .map($map)
            .try_fold( FtpList::default(), |mut list, ritem| {
                let item = ritem?;
                match item.ty {
                    FtpItemType::CurrentDir => {
                        list.current = Some(item);
                    },
                    FtpItemType::ParentDir => {
                        list.parent = Some(item);
                    },
                    _ => {
                        list.items.push(item);
                    },
                };
                Ok(list)
            })
    };
}    

impl FtpClient {
    
    pub fn new(settings: Box<dyn FtpClientSettings>) -> Self {
        Self {
            settings,
            settings_list_mode: None,
            effective_list_mode: None,

            ftp: None,

            has_feat: false,
            features: Default::default(),

            current_path: None,
        }
    }

    fn reconnect(&mut self) -> FtpResult<&mut FtpStream> {
        // drop existing ftp connection
        self.ftp = None;

        let mut ftp = FtpStream::connect(self.settings.addr())?;
        
        if !self.has_feat && self.settings.use_feat() {
            self.features = ftp.feat()?.into();
            self.has_feat = true;
        }

        #[cfg(feature = "secure")]
        if self.settings.use_secure() {
            let sni = self.settings.sni();

            let tls_connector = TlsConnector::builder()
                .use_sni(sni.is_some())
                .build()
                .map_err(|e| FtpError::SecureError(e.to_string()))?;

            ftp = ftp.into_secure(tls_connector, sni.unwrap_or_default())?;
        };

        ftp.login( self.settings.login(), self.settings.password() )?;

        if let Some(path) = self.settings.remote_dir() {
            ftp.cwd(path)?;
        }

        if !self.current_path.is_none() {
//            ftp.cwd(self.current_path.as_str())?;
        }

        self.ftp = Some(ftp);

        Ok(self.ftp.as_mut().unwrap())
    }

    pub fn cdup(&mut self) -> FtpResult<()> {
        ftp!(self, cdup())
    }

    pub fn chdir(&mut self, path: &str) -> FtpResult<()> {
        ftp!(self, cwd(path))
    }

    fn list_mlsd(&mut self) -> FtpResult<FtpList> {
        list_fn!(self, mlsd, |s| parse_mlst_line(s.as_str()).map_err(|_| FtpError::BadResponse))
    }

    fn list_nlst(&mut self) -> FtpResult<FtpList> {
        unimplemented!()
    }

    fn list_stat(&mut self) -> FtpResult<FtpList> {
        unimplemented!()
    }

    fn list_list(&mut self) -> FtpResult<FtpList> {
        list_fn!(self, list, |s| list::File::from_str(s.as_str()).map(|f| list_to_ftp(&f)).map_err(|_| FtpError::BadResponse))
    }

    fn get_list_mode(&mut self) -> FtpClientListMode {
        match self.effective_list_mode {
            Some(lm) => return lm,
            _ => {},
        };

        if self.settings_list_mode.is_none() {
            self.settings_list_mode = self.settings.list_mode();
        };

        match self.settings_list_mode {
            Some(lm) => {
                self.effective_list_mode = self.settings_list_mode;
                return lm;
            },
            _ => {},
        };

        FtpClientListMode::List
    }

    pub fn list(&mut self) -> FtpResult<FtpList> {
        match self.get_list_mode() {
            FtpClientListMode::List => self.list_list(),
            FtpClientListMode::Nlst => self.list_nlst(),
            FtpClientListMode::Mlsd => self.list_mlsd(),
            FtpClientListMode::Stat => self.list_stat(),
        }
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use suppaftp::test::*;

    #[derive(Debug)]
    struct TestSettings {}

    impl FtpClientSettings for TestSettings {
        fn addr(&self) -> &str { TEST_SERVER_ADDR }
        fn login(&self) -> &str { TEST_SERVER_LOGIN }
        fn password(&self) -> &str { TEST_SERVER_PASSWORD }
        fn remote_dir(&self) -> Option<&str> { None }
        
        #[cfg(feature = "secure")]
        fn use_secure(&self) -> bool { false }
    
        #[cfg(feature = "secure")]
        fn sni(&self) -> Option<&str> { None }
    
        fn list_mode(&self) -> Option<FtpClientListMode> {
            Some(FtpClientListMode::Mlsd)
        }
    }

    fn settings() -> Box<dyn FtpClientSettings> {
        Box::new(TestSettings {})
    }

    #[test]
    fn test() {
        let mut client = FtpClient::new(settings());
        dbg!(client.list());
    }
}