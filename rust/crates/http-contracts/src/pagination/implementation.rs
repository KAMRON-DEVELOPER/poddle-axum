use crate::pagination::{error::PaginationError, schema::Pagination};

impl Pagination {
    pub fn validate(&self) -> Result<(), PaginationError> {
        if self.offset < 0 {
            return Err(PaginationError::NegativeOffset);
        }

        if self.limit <= 0 {
            return Err(PaginationError::ZeroOrNegativeLimit);
        }

        if self.limit > 100 {
            return Err(PaginationError::LimitTooLarge);
        }

        Ok(())
    }
}
