use std::fmt::Debug;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("The player is already running")]
pub struct PlayerRunningError;

#[derive(Debug, Error)]
pub enum SeekError {
    #[error("0")]
    OutOfRange(OutOfBoundsError<Duration>),
    // Returned when the Player tries to seek but there is no song playing;
    #[error("The player does not have a song which can be skipped")]
    NoCurrentSong,
}

impl SeekError {
    pub fn out_of_range(to: Duration, max: Duration) -> Self {
        Self::OutOfRange(OutOfBoundsError::High { value: to, max })
    }
}

#[derive(Debug, Error)]
pub enum OutOfBoundsError<T: PartialOrd + Debug> {
    #[error("Expected value less than {min:?}, got {value:?}")]
    Low { value: T, min: T },
    #[error("Expected value higher than {max:?}, got {value:?}")]
    High { value: T, max: T },
    #[error("Expected value in range ({min:?}, {max:?}), got {value:?}")]
    LowHigh { value: T, min: T, max: T },
}
