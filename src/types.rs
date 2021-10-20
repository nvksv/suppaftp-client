use chrono::NaiveDateTime;
use std::collections::HashMap;
use std::convert::{From, TryFrom, TryInto};
use suppaftp::list;
use crate::mlst::MlstFilePermissions;

///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FtpItemType {
    File,
    Dir,
    CurrentDir,
    ParentDir    
}

impl FtpItemType {
    pub fn is_dir(&self) -> bool {
        *self != Self::File
    }
}

impl TryFrom<&str> for FtpItemType {
    type Error = list::ParseError;

    fn try_from(ty: &str) -> std::result::Result<Self, Self::Error> {
        match ty.to_ascii_lowercase().as_str() {
            "file"  => Ok(FtpItemType::File),
            "cdir"  => Ok(FtpItemType::CurrentDir),
            "pdir"  => Ok(FtpItemType::ParentDir),
            "dir"   => Ok(FtpItemType::Dir),
            _ => Err(list::ParseError::SyntaxError),
        }
    }
}

///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FtpItem {
    pub name:               String,
    pub ty:                 FtpItemType,
    pub size:               Option<u64>,
    pub modified:           Option<NaiveDateTime>,
    pub created:            Option<NaiveDateTime>,
    pub unique:             Option<String>,
    pub perm:               Option<MlstFilePermissions>,
    pub lang:               Option<String>,
    pub media_type:         Option<String>,
    pub charset:            Option<String>,
    pub unix_owner:         Option<u32>,
    pub unix_ownername:     Option<String>,
    pub unix_group:         Option<u32>,
    pub unix_groupname:     Option<String>,
    pub unix_mode:          Option<u16>,
    pub others:             Option<HashMap<String, String>>,
}

///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FtpList {
    pub current: Option<FtpItem>,
    pub parent: Option<FtpItem>,
    pub items: Vec<FtpItem>,
}

impl Default for FtpList {
    fn default() -> Self {
        Self {
            current: None,
            parent: None,
            items: vec![],        
        }
    }    
}
