mod raw;

use std::ffi::{c_void, CStr, CString};

use anyhow::Result;

pub type MpvEventID = raw::mpv_event_id;
pub type MpvFormat = raw::mpv_format;
pub type MpvRawHandle = *mut raw::mpv_handle;
pub struct MpvHandle(*mut raw::mpv_handle);
pub struct MpvEvent(*mut raw::mpv_event);
pub struct MpvEventProperty(*mut raw::mpv_event_property);

impl MpvHandle {
    pub fn from_ptr(handle: MpvRawHandle) -> Self {
        assert!(!handle.is_null());
        Self(handle)
    }

    pub fn wait_event(&self, timeout: f64) -> MpvEvent {
        MpvEvent::from_ptr(unsafe { raw::mpv_wait_event(self.0, timeout) })
    }

    pub fn client_name(&self) -> Result<String> {
        Ok(unsafe {
            let c_name = raw::mpv_client_name(self.0);
            let c_str = CStr::from_ptr(c_name);
            let str_slice: &str = c_str.to_str()?;
            str_slice.to_owned()
        })
    }

    pub fn get_property_string<S: Into<String>>(&self, name: S) -> Result<String> {
        let c_name = CString::new(name.into())?;

        Ok(unsafe {
            let c_path = raw::mpv_get_property_string(self.0, c_name.as_ptr());
            // TODO Maybe check if the pointer is not null ?
            let c_str = CStr::from_ptr(c_path);
            let str_slice: &str = c_str.to_str()?;
            let str_buf: String = str_slice.to_owned();
            raw::mpv_free(c_path as *mut c_void);
            str_buf
        })
    }

    pub fn set_property<S: Into<String>, T>(
        &self,
        name: S,
        format: MpvFormat,
        mut data: T,
    ) -> Result<()> {
        let c_name = CString::new(name.into())?;

        if unsafe {
            let data: *mut c_void = &mut data as *mut _ as *mut c_void;
            raw::mpv_set_property(self.0, c_name.as_ptr(), format, data) == 0
        } {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to set property"))
        }
    }

    pub fn observe_property<S: Into<String>>(
        &self,
        reply_userdata: u64,
        name: S,
        format: MpvFormat,
    ) -> Result<()> {
        let c_name = CString::new(name.into())?;

        if unsafe {
            raw::mpv_observe_property(self.0, reply_userdata, c_name.as_ptr(), format) == 0
        } {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to observe property"))
        }
    }
}

impl MpvEvent {
    fn from_ptr(event: *mut raw::mpv_event) -> Self {
        assert!(!event.is_null());
        Self(event)
    }

    pub fn get_event_id(&self) -> MpvEventID {
        unsafe { (*self.0).event_id }
    }

    pub fn get_reply_userdata(&self) -> u64 {
        unsafe { (*self.0).reply_userdata }
    }

    pub fn get_event_property(&self) -> MpvEventProperty {
        MpvEventProperty::from_ptr(unsafe { (*self.0).data as *mut raw::mpv_event_property })
    }
}

impl MpvEventProperty {
    fn from_ptr(event_property: *mut raw::mpv_event_property) -> Self {
        assert!(!event_property.is_null());
        Self(event_property)
    }

    pub fn get_data<T: Copy>(&self) -> Option<T> {
        unsafe {
            let data = (*self.0).data as *mut T;
            if data.is_null() {
                None
            } else {
                Some(*data)
            }
        }
    }
}
