use std::fmt::Debug;
use std::time::Duration;
use thiserror::Error;

/// Returned by [`Player::run`] if the player is still running.
///
/// [`Player::run`]: crate::playback::Player::run
#[derive(Debug, Error)]
pub enum PlayerStartError {
    #[error("The player is already running")]
    Running,
    #[error("The player was started while the queue was empty")]
    EmptyQueue,
}

/// Returned when [`Player::seek_duration`] fails.
///
/// This can be because the duration given was out of bounds, or because the method was called when there was no song playing
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
    Range { value: T, min: T, max: T },
}

impl<T: PartialOrd + Debug> OutOfBoundsError<T> {
    pub fn low(value: T, min: T) -> Self {
        Self::Low { value, min }
    }

    pub fn high(value: T, max: T) -> Self {
        Self::High { value, max }
    }

    pub fn range(value: T, min: T, max: T) -> Self {
        Self::Range { value, min, max }
    }
}


#[derive(Debug, Error)]
pub enum StreamSetupError {
    #[error("Unsupported sample format {0}")]
    UnsupportedSampleFormat(cpal::SampleFormat),
    #[error("Failed to build stream: {0}")]
    BuildStreamError(#[from] cpal::BuildStreamError),
    #[error("Found no default audio device")]
    NoDeviceFound,
}