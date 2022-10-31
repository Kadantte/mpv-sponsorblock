mod error;
mod ffi;
pub mod format;

use error::Error;
use ffi::*;
use format::Format;

use std::ffi::{c_void, CStr, CString};
use std::fmt;

pub type RawHandle = *mut mpv_handle;

pub struct Handle(*mut mpv_handle);

pub struct EventStartFile(*mut mpv_event_start_file);
pub struct EventProperty(*mut mpv_event_property);
pub struct EventHook(*mut mpv_event_hook);

pub type Result<T> = std::result::Result<T, Error>;

macro_rules! mpv_result {
    ($f:expr) => {
        unsafe {
            match $f {
                mpv_error::SUCCESS => Ok(()),
                e => Err(Error::new(e)),
            }
        }
    };
}

pub enum Event {
    None,
    Shutdown,
    LogMessage, // TODO mpv_event_log_message
    GetPropertyReply(EventProperty),
    SetPropertyReply,
    CommandReply, // TODO mpv_event_command
    StartFile(EventStartFile),
    EndFile, // TODO mpv_event_end_file
    FileLoaded,
    ClientMessage, // TODO mpv_event_client_message
    VideoReconfig,
    AudioReconfig,
    Seek,
    PlaybackRestart,
    PropertyChange(EventProperty),
    QueueOverflow,
    Hook(EventHook),
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::None => write!(f, "none"),
            Self::Shutdown => write!(f, "shutdown"),
            Self::LogMessage => write!(f, "log message"),
            Self::GetPropertyReply(ref event) => write!(f, "get property reply [{}]", event.get_name()),
            Self::SetPropertyReply => write!(f, "set property reply"),
            Self::CommandReply => write!(f, "command reply"),
            Self::StartFile(ref event) => write!(f, "start file [{}]", event.get_playlist_entry_id()),
            Self::EndFile => write!(f, "end file"),
            Self::FileLoaded => write!(f, "file loaded"),
            Self::ClientMessage => write!(f, "client message"),
            Self::VideoReconfig => write!(f, "video reconfig"),
            Self::AudioReconfig => write!(f, "audio reconfig"),
            Self::Seek => write!(f, "seek"),
            Self::PlaybackRestart => write!(f, "playback restart"),
            Self::PropertyChange(ref event) => write!(f, "property change [{}]", event.get_name()),
            Self::QueueOverflow => write!(f, "queue overflow"),
            Self::Hook(ref event) => write!(f, "hook [{}]", event.get_name()),
        }
    }
}

impl Handle {
    pub fn from_ptr(handle: RawHandle) -> Self {
        assert!(!handle.is_null());
        Self(handle)
    }

    pub fn wait_event(&self, timeout: f64) -> (u64, Result<Event>) {
        unsafe {
            let mpv_event = mpv_wait_event(self.0, timeout);

            if mpv_event.is_null() {
                return (0, Ok(Event::None));
            }

            let mpv_reply: u64 = (*mpv_event).reply_userdata;

            if (*mpv_event).error != mpv_error::SUCCESS {
                return (mpv_reply, Err(Error::new((*mpv_event).error)));
            }

            (
                mpv_reply,
                Ok(match (*mpv_event).event_id {
                    mpv_event_id::SHUTDOWN => Event::Shutdown,
                    mpv_event_id::LOG_MESSAGE => Event::LogMessage,
                    mpv_event_id::GET_PROPERTY_REPLY => {
                        Event::GetPropertyReply(EventProperty::from_ptr((*mpv_event).data as *mut mpv_event_property))
                    }
                    mpv_event_id::SET_PROPERTY_REPLY => Event::SetPropertyReply,
                    mpv_event_id::COMMAND_REPLY => Event::CommandReply,
                    mpv_event_id::START_FILE => {
                        Event::StartFile(EventStartFile::from_ptr((*mpv_event).data as *mut mpv_event_start_file))
                    }
                    mpv_event_id::END_FILE => Event::EndFile,
                    mpv_event_id::FILE_LOADED => Event::FileLoaded,
                    mpv_event_id::CLIENT_MESSAGE => Event::ClientMessage,
                    mpv_event_id::VIDEO_RECONFIG => Event::VideoReconfig,
                    mpv_event_id::AUDIO_RECONFIG => Event::AudioReconfig,
                    mpv_event_id::SEEK => Event::Seek,
                    mpv_event_id::PLAYBACK_RESTART => Event::PlaybackRestart,
                    mpv_event_id::PROPERTY_CHANGE => {
                        Event::PropertyChange(EventProperty::from_ptr((*mpv_event).data as *mut mpv_event_property))
                    }
                    mpv_event_id::QUEUE_OVERFLOW => Event::QueueOverflow,
                    mpv_event_id::HOOK => Event::Hook(EventHook::from_ptr((*mpv_event).data as *mut mpv_event_hook)),
                    _ => Event::None,
                }),
            )
        }
    }

    pub fn client_name(&self) -> &str {
        unsafe { CStr::from_ptr(mpv_client_name(self.0)) }
            .to_str()
            .unwrap_or("unknown")
    }

    pub fn command(&self, args: Vec<String>) -> Result<()> {
        let c_args = args
            .iter()
            .map(|s| CString::new::<String>(s.into()).unwrap()) // Dangerous
            .collect::<Vec<CString>>();
        let mut raw_args = c_args.iter().map(|s| s.as_ptr()).collect::<Vec<_>>();
        raw_args.push(std::ptr::null::<i8>()); // Adding null at the end

        mpv_result!(mpv_command(self.0, raw_args.as_ptr()))
    }

    /// This is garbage
    pub fn set_property<T: 'static + Format, S: Into<String>>(&self, name: S, mut data: T) -> Result<()> {
        let p_data: *mut c_void = &mut data as *mut _ as *mut c_void;
        mpv_result!(mpv_set_property(
            self.0,
            CString::new(name.into())?.as_ptr(),
            T::get_format(),
            p_data
        ))
    }

    /// This is garbage
    /*pub fn get_property<T: 'static + Format + Default, S: Into<String>>(&self, name: S) -> Result<T> {
        let c_name = CString::new(name.into())?;
        let data = T::default();
        let p_data: data.;
        assert!(p_data.is_null());
        mpv_result!(mpv_get_property(self.0, c_name.as_ptr(), T::get_format(), p_data))?;
        T::from_raw(p_data).ok_or(Error::new(mpv_error::GENERIC))
    }*/

    /// This is garbage
    pub fn get_property_string<S: Into<String>>(&self, name: S) -> Result<String> {
        let c_name = CString::new(name.into())?;

        unsafe {
            let c_path = mpv_get_property_string(self.0, c_name.as_ptr());
            if c_path.is_null() {
                return Err(Error::new(mpv_error::PROPERTY_NOT_FOUND));
            }

            let c_str = CStr::from_ptr(c_path);
            let str_buf = c_str.to_str().map(|s| s.to_owned());
            mpv_free(c_path as *mut c_void);
            Ok(str_buf?) // Meh it shouldn't fail
        }
    }

    pub fn observe_property<S: Into<String>>(&self, reply_userdata: u64, name: S, format: mpv_format) -> Result<()> {
        mpv_result!(mpv_observe_property(
            self.0,
            reply_userdata,
            CString::new(name.into())?.as_ptr(),
            format
        ))
    }

    pub fn hook_add<S: Into<String>>(&self, reply_userdata: u64, name: S, priority: i32) -> Result<()> {
        mpv_result!(mpv_hook_add(
            self.0,
            reply_userdata,
            CString::new(name.into())?.as_ptr(),
            priority
        ))
    }

    pub fn hook_continue(&self, id: u64) -> Result<()> {
        mpv_result!(mpv_hook_continue(self.0, id))
    }
}

impl EventStartFile {
    fn from_ptr(event: *mut mpv_event_start_file) -> Self {
        assert!(!event.is_null());
        Self(event)
    }

    pub fn get_playlist_entry_id(&self) -> u64 {
        unsafe { (*self.0).playlist_entry_id }
    }
}

impl EventProperty {
    fn from_ptr(event: *mut mpv_event_property) -> Self {
        assert!(!event.is_null());
        Self(event)
    }

    pub fn get_name(&self) -> &str {
        unsafe { CStr::from_ptr((*self.0).name) }.to_str().unwrap_or("unknown")
    }

    pub fn get_data<T: Format>(&self) -> Option<T> {
        unsafe {
            if (*self.0).format == T::get_format() {
                T::from_raw((*self.0).data)
            } else {
                None
            }
        }
    }
}

impl EventHook {
    fn from_ptr(event: *mut mpv_event_hook) -> Self {
        assert!(!event.is_null());
        Self(event)
    }

    pub fn get_name(&self) -> &str {
        unsafe { CStr::from_ptr((*self.0).name) }.to_str().unwrap_or("unknown")
    }

    pub fn get_id(&self) -> u64 {
        unsafe { (*self.0).id }
    }
}
