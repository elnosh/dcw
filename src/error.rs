use std::fmt;

#[derive(Debug)]
pub enum WalletError {
    CashuCrabErr(cashu_crab::error::Error),
    ClientErr(cashu_crab::client::Error),
    SledErr(sled::Error),
    MinReqErr(minreq::Error),
    ParseErr(url::ParseError),
    SerdeJsonErr(serde_json::Error),
    InsufficientFunds,
    InvoiceNotFound,
    WalletSetupErr,
}

impl From<cashu_crab::client::Error> for WalletError {
    fn from(err: cashu_crab::client::Error) -> Self {
        WalletError::ClientErr(err)
    }
}

impl From<cashu_crab::error::Error> for WalletError {
    fn from(err: cashu_crab::error::Error) -> Self {
        WalletError::CashuCrabErr(err)
    }
}

impl From<minreq::Error> for WalletError {
    fn from(err: minreq::Error) -> Self {
        WalletError::MinReqErr(err)
    }
}

impl From<sled::Error> for WalletError {
    fn from(err: sled::Error) -> Self {
        WalletError::SledErr(err)
    }
}

impl From<url::ParseError> for WalletError {
    fn from(err: url::ParseError) -> Self {
        WalletError::ParseErr(err)
    }
}

impl From<serde_json::Error> for WalletError {
    fn from(err: serde_json::Error) -> Self {
        WalletError::SerdeJsonErr(err)
    }
}

impl fmt::Display for WalletError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WalletError::CashuCrabErr(err) => write!(f, "{}", err),
            WalletError::ClientErr(err) => write!(f, "{}", err),
            WalletError::SledErr(err) => write!(f, "{}", err),
            WalletError::MinReqErr(err) => write!(f, "{}", err),
            WalletError::ParseErr(err) => write!(f, "{}", err),
            WalletError::SerdeJsonErr(err) => write!(f, "{}", err),
            WalletError::InsufficientFunds => write!(f, "insufficient funds"),
            WalletError::InvoiceNotFound => write!(f, "invoice not found"),
            WalletError::WalletSetupErr => write!(f, "error setting up wallet"),
        }
    }
}
