use crate::chain::cli::version::major_version;
use crate::chain::driver::ChainDriver;
use crate::error::Error;
use crate::types::tagged::*;

pub trait ChainVersionMethodsExt {
    fn major_version(&self) -> Result<u64, Error>;
}

impl<Chain: Send> ChainVersionMethodsExt for MonoTagged<Chain, &ChainDriver> {
    fn major_version(&self) -> Result<u64, Error> {
        major_version(&self.value().command_path)
    }
}
