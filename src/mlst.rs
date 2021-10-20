use chrono::{NaiveDateTime, NaiveDate, NaiveTime, Local, TimeZone, DateTime};
use std::collections::HashMap;
use std::convert::{From, TryFrom, TryInto};
use std::time::SystemTime;

use suppaftp::list;
use crate::types::{FtpItem, FtpItemType};

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct MlstFilePermissions {
    pub append: bool,
    pub create: bool,
    pub delete: bool,
    pub enter:  bool,
    pub rename: bool,
    pub list:   bool,
    pub mkdir:  bool,
    pub purge:  bool,
    pub read:   bool,
    pub write:  bool,
}

impl TryFrom<&str> for MlstFilePermissions {
    type Error = list::ParseError;

    fn try_from(fact_value: &str) -> std::result::Result<Self, Self::Error> {
        let mut perm: MlstFilePermissions = Default::default();
        
        for ch in fact_value.chars() {
            match ch.to_ascii_lowercase() {
                'a' => {
                    perm.append = true;
                },
                'c' => {
                    perm.create = true;
                },
                'd' => {
                    perm.delete = true;
                },
                'e' => {
                    perm.enter = true;
                },
                'f' => {
                    perm.rename = true;
                },
                'l' => {
                    perm.list = true;
                },
                'm' => {
                    perm.mkdir = true;
                },
                'p' => {
                    perm.purge = true;
                },
                'r' => {
                    perm.read = true;
                },
                'w' => {
                    perm.write = true;
                },
                _ => {
                    return Err(list::ParseError::SyntaxError);
                }
            }
        };

        Ok(perm)
    }
}

impl MlstFilePermissions {
    pub fn as_pex(&self) -> u8 {
        (if self.read  {4} else {0}) + 
        (if self.write {2} else {0}) + 
        (if self.list  {1} else {0})    
    }
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum MlstFact {
    Other(String),
    Ty,
    Size,
    Modify,
    Create,
    Unique,
    Perm,
    Lang,              
    MediaType,
    Charset,
    UnixOwner,
    UnixOwnerName,
    UnixGroup,
    UnixGroupName,
    UnixMode,         
}

impl From<&str> for MlstFact {
    fn from(name: &str) -> Self {
        let name = name.to_ascii_lowercase();
        match name.as_str() {
            "size" => MlstFact::Size,
            "modify" => MlstFact::Modify,
            "create" => MlstFact::Create,
            "type" => MlstFact::Ty,
            "unique" => MlstFact::Unique,
            "perm" => MlstFact::Perm,
            "lang" => MlstFact::Lang,
            "media-type" => MlstFact::MediaType,
            "charset" => MlstFact::Charset,
            "unix.owner" => MlstFact::UnixOwner,
            "unix.ownername" => MlstFact::UnixOwnerName,
            "unix.group" => MlstFact::UnixGroup,
            "unix.groupname" => MlstFact::UnixGroupName,
            "unix.mode" => MlstFact::UnixMode,
            _ => MlstFact::Other(name)
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

pub fn parse_mlst_date(s: &str) -> Option<NaiveDateTime> {

    let has_ms;

    match s.len() {
        14 => {
            has_ms = false;
        },
        18 => {
            has_ms = true;
        },
        _ => {
            return None;
        }
    }

    if !s.chars().enumerate().all(|(i,ch)| {
        if has_ms && i == 14 {
            ch == '.'
        } else {
            ch.is_ascii_digit()
        }
    }) {
        return None;
    }

    // now it's safe to use indexes

    let year:   i32 = s[0..4].parse().ok()?;
    let month:  u32 = s[4..6].parse().ok()?;
    let day:    u32 = s[6..8].parse().ok()?;
    let hour:   u32 = s[8..10].parse().ok()?;
    let minute: u32 = s[10..12].parse().ok()?;
    let second: u32 = s[12..14].parse().ok()?;

    let dt;

    if has_ms {
        let ms: u32 = s[15..18].parse().ok()?;
        
        dt = NaiveDateTime::new(NaiveDate::from_ymd(year, month, day), NaiveTime::from_hms_milli(hour, minute, second, ms));
    } else {
        dt = NaiveDateTime::new(NaiveDate::from_ymd(year, month, day), NaiveTime::from_hms(hour, minute, second));
    };

    Some(dt)
}

pub fn parse_mlst_feat(line: &str) -> Vec<(MlstFact, bool)> {
    line.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            let (s, opt) = match s.rsplit_once('*') {
                Some((s, empty)) if empty.is_empty() => (s, true),
                _ => (s, false),
            };
            (s.into(), opt)
        })
        .collect()
}

pub fn parse_mlst_line(line: &str) -> Result<FtpItem, list::ParseError> {

    let mut file_ty:        Option<_> = None;
    let mut file_size:      Option<_> = None;
    let mut file_modify:    Option<_> = None;
    let mut file_create:    Option<_> = None;
    let mut file_unique:    Option<_> = None;
    let mut file_perm:      Option<_> = None;
    let mut file_lang:      Option<_> = None;
    let mut file_media_type:    Option<_> = None;
    let mut file_charset:       Option<_> = None;
    let mut file_unix_owner:    Option<_> = None;
    let mut file_unix_ownername:    Option<_> = None;
    let mut file_unix_group:        Option<_> = None;
    let mut file_unix_groupname:    Option<_> = None;
    let mut file_unix_mode:     Option<_> = None;
    let mut file_others:        Option<_> = None;

    let mut fact_name   = String::with_capacity(20);
    let mut fact_value  = String::with_capacity(20);

    enum FSM {
        Name,
        Value,
    }

    const SPACE:        char = ' ';
    const EQUAL:        char = '=';
    const SEMICOLON:    char = ';';

    let mut state = FSM::Name;

    let mut chars = line.chars();

    while let Some(ch) = chars.next() {
        match state {
            FSM::Name => {
                if ch == SPACE {
                    if fact_name.is_empty() {
                        break;
                    } else {
                        return Err(list::ParseError::SyntaxError);
                    }
                } else if ch == SEMICOLON {
                    return Err(list::ParseError::SyntaxError);
                } else if ch == EQUAL {
                    state = FSM::Value;
                    continue;
                } else {
                    fact_name.push(ch);
                    continue;
                }
            },
            FSM::Value => {
                if ch == SPACE || ch == EQUAL {
                    return Err(list::ParseError::SyntaxError);
                } else if ch == SEMICOLON {
                    // do nothing, just move on
                } else {
                    fact_value.push(ch);
                    continue;
                }

                if fact_name.is_empty() {
                    return Err(list::ParseError::SyntaxError);
                }

                match fact_name.as_str().into() {
                    MlstFact::Size => {
                        file_size = Some(fact_value.parse().map_err(|_| list::ParseError::BadSize)?);
                    },
                    MlstFact::Modify => {
                        file_modify = Some(parse_mlst_date(&fact_value).ok_or(list::ParseError::InvalidDate)?);
                    },
                    MlstFact::Create => {
                        file_create = Some(parse_mlst_date(&fact_value).ok_or(list::ParseError::InvalidDate)?);
                    },
                    MlstFact::Ty => {
                        file_ty = Some(fact_value.as_str().try_into()?);
                    },
                    MlstFact::Unique => {
                        file_unique = Some(fact_value.clone());
                    },
                    MlstFact::Perm => {
                        file_perm = Some(fact_value.as_str().try_into()?);
                    },
                    MlstFact::Lang => {
                        file_lang = Some(fact_value.clone());
                    },
                    MlstFact::MediaType => {
                        file_media_type = Some(fact_value.clone());
                    },
                    MlstFact::Charset => {
                        file_charset = Some(fact_value.clone());
                    },
                    MlstFact::UnixOwner => {
                        file_unix_owner = Some(fact_value.parse().map_err(|_| list::ParseError::SyntaxError)?);
                    },
                    MlstFact::UnixOwnerName => {
                        file_unix_ownername = Some(fact_value.clone());
                    },
                    MlstFact::UnixGroup => {
                        file_unix_group = Some(fact_value.parse().map_err(|_| list::ParseError::SyntaxError)?);
                    },
                    MlstFact::UnixGroupName => {
                        file_unix_groupname = Some(fact_value.clone());
                    },
                    MlstFact::UnixMode => {
                        file_unix_mode = Some(u16::from_str_radix(&fact_value, 8).map_err(|_| list::ParseError::SyntaxError)?);
                    },
                    MlstFact::Other(fact_name) => {
                        file_others.get_or_insert_with(|| HashMap::new()).insert( fact_name, fact_value.clone() );
                    }
                }

                fact_name.clear();
                fact_value.clear();
                state = FSM::Name;
                continue;
            }
        }
    }

    let name: String = chars.collect();
    if name.is_empty() {
        return Err(list::ParseError::SyntaxError);
    }

    let ty_unwrapped = file_ty.ok_or(list::ParseError::SyntaxError)?;

    let file = FtpItem {
        name,
        ty: ty_unwrapped,
        size:   file_size,
        modified: file_modify,
        created: file_create,
        unique: file_unique,
        perm:   file_perm,
        lang:   file_lang,
        media_type: file_media_type,
        charset:    file_charset,
        unix_owner: file_unix_owner,
        unix_ownername: file_unix_ownername,
        unix_group:     file_unix_group,
        unix_groupname: file_unix_groupname,
        unix_mode:  file_unix_mode,
        others:     file_others,        
    };

    Ok(file)
}

fn systemtime_to_naivedatetime( t: SystemTime ) -> NaiveDateTime {
    let dt: DateTime<Local> = t.into();
    dt.naive_local()    
}

fn naivedatetime_to_systemtime( t: NaiveDateTime ) -> SystemTime {
    Local.from_local_datetime(&t).unwrap().into()
}

pub fn ftp_to_list( file: FtpItem ) -> list::File {
    let is_dir      = file.ty.is_dir();
    let size        = file.size.unwrap_or(0);
    let modified    = naivedatetime_to_systemtime( file.modified.unwrap_or(NaiveDateTime::from_timestamp(0, 0)) );
    let pex         = file.perm.as_ref().map(MlstFilePermissions::as_pex).unwrap_or(0);

    list::File::from_raw(file.name, is_dir, size as usize, modified, file.unix_owner, file.unix_group, (pex, pex, pex))
}

macro_rules! mode_bits {
    ($file: expr, $who: ident, $access: ident) => {
        if $file.$access(list::PosixPexQuery::$who) {1} else {0}
    };
    ($file: expr, $who: ident) => {
            (mode_bits!($file, $who, can_read) << 2)
        |   (mode_bits!($file, $who, can_write) << 1)
        |   mode_bits!($file, $who, can_execute)
    };
    ($file: expr) => {
            (mode_bits!($file, Others) << 6)
        |   (mode_bits!($file, Group) << 3)
        |   mode_bits!($file, Owner)
};
}

pub fn list_to_ftp( file: &list::File ) -> FtpItem {

    let name    = file.name().to_string(); 
    let ty      = if file.is_directory() {
        match file.name() {
            "." => FtpItemType::CurrentDir,
            ".." => FtpItemType::ParentDir,
            _ => FtpItemType::Dir
        }
    } else {
        FtpItemType::File
    };
    let size    = Some(file.size() as u64);
    let modified  = Some(systemtime_to_naivedatetime(file.modified()));

    let mut perm: MlstFilePermissions = Default::default();
    perm.read   = file.can_read(list::PosixPexQuery::Owner);
    perm.write  = file.can_write(list::PosixPexQuery::Owner);
    perm.list   = file.can_execute(list::PosixPexQuery::Owner);

    let unix_mode = Some(mode_bits!(file));

    FtpItem {
        name,
        ty,
        size,
        modified,
        created: None,
        unique: None,
        perm:   Some(perm),
        lang:   None,
        media_type: None,
        charset:    None,
        unix_owner: file.uid(),
        unix_ownername: None,
        unix_group:     file.gid(),
        unix_groupname: None,
        unix_mode,
        others: None, 
    }
}

impl From<FtpItem> for list::File {
    fn from(file: FtpItem) -> Self {
        ftp_to_list(file)    
    }
}

impl From<list::File> for FtpItem {
    fn from(file: list::File) -> Self {
        list_to_ftp(&file)    
    }
}