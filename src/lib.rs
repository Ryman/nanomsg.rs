#![feature(globs, unsafe_destructor, phase, slicing_syntax)]

#[phase(plugin, link)] extern crate log;

extern crate libc;

extern crate libnanomsg;

pub use result::{NanoResult, NanoError, NanoErrorKind};
pub use endpoint::{Endpoint};

use libnanomsg::nn_pollfd;

use libc::{c_int, c_void, size_t};
use std::mem::transmute;
use std::ptr;
use result::*;
use std::io::{Writer, Reader, IoResult};
use std::io;
use std::mem::size_of;
use std::time::duration::Duration;
use std::kinds::marker::ContravariantLifetime;
use std::vec::Vec;
use std::c_vec::CVec;

mod result;
mod endpoint;

/// Type-safe protocols that Nanomsg uses. Each socket
/// is bound to a single protocol that has specific behaviour
/// (such as only being able to receive messages and not send 'em).
#[deriving(Show, PartialEq)]
pub enum Protocol {
    Req = (libnanomsg::NN_REQ) as int,
    Rep = (libnanomsg::NN_REP) as int,
    Push = (libnanomsg::NN_PUSH) as int,
    Pull = (libnanomsg::NN_PULL) as int,
    Pair = (libnanomsg::NN_PAIR) as int,
    Bus = (libnanomsg::NN_BUS) as int,
    Pub = (libnanomsg::NN_PUB) as int,
    Sub = (libnanomsg::NN_SUB) as int,
    Surveyor = (libnanomsg::NN_SURVEYOR) as int,
    Respondent = (libnanomsg::NN_RESPONDENT) as int
}

impl Protocol {
    fn to_raw(&self) -> c_int{
        *self as c_int
    }
}

/// A type-safe socket wrapper around nanomsg's own socket implementation. This
/// provides a safe interface for dealing with initializing the sockets, sending
/// and receiving messages.
pub struct Socket<'a> {
    socket: c_int,
    marker: ContravariantLifetime<'a>
}

pub struct PollFd {
    socket: c_int,
    check_pollin: bool,
    check_pollout: bool,
    check_pollin_result: bool,
    check_pollout_result: bool
}

impl PollFd {

    fn convert(&self) -> nn_pollfd {
        nn_pollfd::new(self.socket, self.check_pollin, self.check_pollout)
    }

    pub fn can_read(&self) -> bool {
        self.check_pollin_result
    }

    pub fn can_write(&self) -> bool {
        self.check_pollout_result
    }

}

pub struct PollRequest<'a> {
    fds: &'a mut [PollFd],
    nn_fds: Vec<nn_pollfd>
}

impl<'a> PollRequest<'a> {
    pub fn new(fds: &'a mut [PollFd]) -> PollRequest<'a> {
        let len = fds.len();
        let nn_fds = Vec::from_fn(len, |idx| fds[idx].convert());
        PollRequest { fds: fds, nn_fds: nn_fds }
    }

    fn len(&self) -> uint {
        self.fds.len()
    }

    pub fn get_fds(&'a self) -> &'a [PollFd] {
        self.fds
    }

    fn get_nn_fds(&mut self) -> *mut nn_pollfd {
        self.nn_fds.as_mut_ptr()
    }

    fn copy_poll_result(&mut self, count: uint) {
 
        for x in range(0, count) {
            self.fds[x].check_pollin_result = self.nn_fds[x].pollin_result();
            self.fds[x].check_pollout_result = self.nn_fds[x].pollout_result();
        }

    }
}

impl<'a> Socket<'a> {

    /// Allocate and initialize a new Nanomsg socket which returns
    /// a new file descriptor behind the scene. The safe interface doesn't
    /// expose any of the underlying file descriptors and such.
    ///
    /// Usage:
    ///
    /// ```rust
    /// use nanomsg::{Socket, Protocol};
    ///
    /// let mut socket = match Socket::new(Protocol::Pull) {
    ///     Ok(socket) => socket,
    ///     Err(err) => panic!("{}", err)
    /// };
    /// ```
    #[unstable]
    pub fn new(protocol: Protocol) -> NanoResult<Socket<'a>> {
        Socket::create_socket(libnanomsg::AF_SP, protocol)
    }

    #[unstable]
    pub fn new_for_device(protocol: Protocol) -> NanoResult<Socket<'a>> {
        Socket::create_socket(libnanomsg::AF_SP_RAW, protocol)
    }

    fn create_socket(domain: c_int, protocol: Protocol) -> NanoResult<Socket<'a>> {
        let socket = unsafe {
            libnanomsg::nn_socket(domain, protocol.to_raw())
        };

        if socket == -1 {
            return Err(last_nano_error());
        }

        debug!("Initialized a new socket");

        Ok(Socket {
            socket: socket,
            marker: ContravariantLifetime::<'a>
        })
    }

    /// Creating a new socket through `Socket::new` does **not**
    /// bind that socket to a listening state. Instead, one has to be
    /// explicit in enabling the socket to listen onto a specific address.
    ///
    /// That's what the `bind` method does. Passing in a raw string like:
    /// "ipc:///tmp/pipeline.ipc" is supported.
    ///
    /// Note: This does **not** block the current task. That job
    /// is up to the user of the library by entering a loop.
    ///
    /// Usage:
    ///
    /// ```rust
    /// use nanomsg::{Socket, Protocol};
    ///
    /// let mut socket = match Socket::new(Protocol::Pull) {
    ///     Ok(socket) => socket,
    ///     Err(err) => panic!("{}", err)
    /// };
    ///
    /// // Bind the newly created socket to the following address:
    /// //match socket.bind("ipc:///tmp/pipeline.ipc") {
    /// //    Ok(_) => {},
    /// //   Err(err) => panic!("Failed to bind socket: {}", err)
    /// //}
    /// ```
    #[unstable]
    pub fn bind<'b, 'a: 'b>(&mut self, addr: &str) -> NanoResult<Endpoint<'b>> {
        let ret = unsafe { libnanomsg::nn_bind(self.socket, addr.to_c_str().as_ptr() as *const i8) };

        if ret == -1 {
            return Err(last_nano_error());
        }

        Ok(Endpoint::new(ret, self.socket))
    }

    #[unstable]
    pub fn connect<'b, 'a: 'b>(&mut self, addr: &str) -> NanoResult<Endpoint<'b>> {
        let ret = unsafe { libnanomsg::nn_connect(self.socket, addr.to_c_str().as_ptr() as *const i8) };

        if ret == -1 {
            return Err(last_nano_error());
        }

        Ok(Endpoint::new(ret, self.socket))
    }

    #[unstable]
    pub fn nb_read(&mut self, buf: &mut [u8]) -> NanoResult<uint> {

        let buf_len = buf.len() as size_t;
        let buf_ptr = buf.as_mut_ptr();
        let c_buf_ptr = buf_ptr as *mut c_void;
        let ret = unsafe { libnanomsg::nn_recv(self.socket, c_buf_ptr, buf_len, libnanomsg::NN_DONTWAIT) };

        if ret == -1 {
            return Err(last_nano_error());
        }

        Ok(ret as uint)
    }

    #[unstable]
    pub fn nb_read_to_end(&mut self) -> NanoResult<Vec<u8>> {
        let mut mem : *mut u8 = ptr::null_mut();

        let ret = unsafe {
            libnanomsg::nn_recv(
                self.socket, 
                transmute(&mut mem),
                libnanomsg::NN_MSG, 
                libnanomsg::NN_DONTWAIT)
        };

        if ret == -1 {
            return Err(last_nano_error());
        }

        let len = ret as uint;
        let c_bytes = unsafe { CVec::new(mem, len) };
        let mut bytes: Vec<u8> = Vec::with_capacity(len);

        bytes.push_all(c_bytes.as_slice());

        unsafe { libnanomsg::nn_freemsg(mem as *mut c_void) };

        Ok(bytes)
    }

    #[unstable]
    pub fn nb_write(&mut self, buf: &[u8]) -> NanoResult<()> {
        let len = buf.len();
        let ret = unsafe {
            libnanomsg::nn_send(
                self.socket, 
                buf.as_ptr() as *const c_void,
                len as size_t, 
                libnanomsg::NN_DONTWAIT)
        };

        if ret == -1 {
            return Err(last_nano_error());
        }

        Ok(())
    }

    #[unstable]
    pub fn new_pollfd(&self, pollin: bool , pollout: bool) -> PollFd {
        PollFd {
            socket: self.socket,
            check_pollin: pollin,
            check_pollout: pollout,
            check_pollin_result: false,
            check_pollout_result: false
        }
    }

    #[unstable]
    pub fn poll(request: &mut PollRequest, timeout: &Duration) -> NanoResult<int> {
        let nn_fds = request.get_nn_fds();
        let len = request.len() as c_int;
        let millis = timeout.num_milliseconds() as c_int;
        let ret = unsafe { libnanomsg::nn_poll(nn_fds, len, millis) } as int;

        if ret == -1 {
            return Err(last_nano_error());
        }

        if ret == 0 {
            return Err(NanoError::new("Timeout", NanoErrorKind::Timeout));
        }

        request.copy_poll_result(ret as uint);

        Ok(ret)
    }

    #[unstable]
    pub fn device(socket1: &Socket, socket2: &Socket) -> NanoResult<()> {
        let ret = unsafe { libnanomsg::nn_device(socket1.socket, socket2.socket) };

        if ret == -1 {
            return Err(last_nano_error());
        }

        Ok(())
    }

    #[unstable]
    pub fn terminate() {
        unsafe { libnanomsg::nn_term() };
    }
    
    #[unstable]
    pub fn set_linger(&mut self, linger: &Duration) -> NanoResult<()> {
        let milliseconds = linger.num_milliseconds();
        let c_linger = milliseconds as c_int;
        let c_linger_ptr = &c_linger as *const _ as *const c_void;
        let ret = unsafe { ;
            libnanomsg::nn_setsockopt (
                self.socket, 
                libnanomsg::NN_SOL_SOCKET, 
                libnanomsg::NN_LINGER, 
                c_linger_ptr, 
                size_of::<c_int>() as size_t) 
        };
 
        if ret == -1 {
            return Err(last_nano_error());
        }

        Ok(())
    }

    #[unstable]
    pub fn set_send_buffer_size(&mut self, size_in_bytes: int) -> NanoResult<()> {
        let c_size_in_bytes = size_in_bytes as c_int;
        let c_size_ptr = &c_size_in_bytes as *const _ as *const c_void;
        let ret = unsafe { 
            libnanomsg::nn_setsockopt (
                self.socket, 
                libnanomsg::NN_SOL_SOCKET, 
                libnanomsg::NN_SNDBUF, 
                c_size_ptr, 
                size_of::<c_int>() as size_t) 
        };
 
        if ret == -1 {
            return Err(last_nano_error());
        }

        Ok(())
    }

    #[unstable]
    pub fn set_receive_buffer_size(&mut self, size_in_bytes: int) -> NanoResult<()> {
        let c_size_in_bytes = size_in_bytes as c_int;
        let c_size_ptr = &c_size_in_bytes as *const _ as *const c_void;
        let ret = unsafe { 
            libnanomsg::nn_setsockopt (
                self.socket, 
                libnanomsg::NN_SOL_SOCKET, 
                libnanomsg::NN_RCVBUF, 
                c_size_ptr, 
                size_of::<c_int>() as size_t) 
        };
 
        if ret == -1 {
            return Err(last_nano_error());
        }

        Ok(())
    }

    #[unstable]
    pub fn set_send_timeout(&mut self, timeout: &Duration) -> NanoResult<()> {
        let milliseconds = timeout.num_milliseconds();
        let c_timeout = milliseconds as c_int;
        let c_timeout_ptr = &c_timeout as *const _ as *const c_void;
        let ret = unsafe { 
            libnanomsg::nn_setsockopt (
                self.socket, 
                libnanomsg::NN_SOL_SOCKET, 
                libnanomsg::NN_SNDTIMEO, 
                c_timeout_ptr, 
                size_of::<c_int>() as size_t) 
        };
 
        if ret == -1 {
            return Err(last_nano_error());
        }

        Ok(())
    }

    #[unstable]
    pub fn set_receive_timeout(&mut self, timeout: &Duration) -> NanoResult<()> {
        let milliseconds = timeout.num_milliseconds();
        let c_timeout = milliseconds as c_int;
        let c_timeout_ptr = &c_timeout as *const _ as *const c_void;
        let ret = unsafe { 
            libnanomsg::nn_setsockopt (
                self.socket, 
                libnanomsg::NN_SOL_SOCKET, 
                libnanomsg::NN_RCVTIMEO, 
                c_timeout_ptr, 
                size_of::<c_int>() as size_t) 
        };
 
        if ret == -1 {
            return Err(last_nano_error());
        }

        Ok(())
    }

    #[unstable]
    pub fn set_reconnect_interval(&mut self, interval: &Duration) -> NanoResult<()> {
        let milliseconds = interval.num_milliseconds();
        let c_interval = milliseconds as c_int;
        let c_interval_ptr = &c_interval as *const _ as *const c_void;
        let ret = unsafe { 
            libnanomsg::nn_setsockopt (
                self.socket, 
                libnanomsg::NN_SOL_SOCKET, 
                libnanomsg::NN_RECONNECT_IVL, 
                c_interval_ptr, 
                size_of::<c_int>() as size_t) 
        };
 
        if ret == -1 {
            return Err(last_nano_error());
        }

        Ok(())
    }

    #[unstable]
    pub fn set_max_reconnect_interval(&mut self, interval: &Duration) -> NanoResult<()> {
        let milliseconds = interval.num_milliseconds();
        let c_interval = milliseconds as c_int;
        let c_interval_ptr = &c_interval as *const _ as *const c_void;
        let ret = unsafe { 
            libnanomsg::nn_setsockopt (
                self.socket, 
                libnanomsg::NN_SOL_SOCKET, 
                libnanomsg::NN_RECONNECT_IVL_MAX, 
                c_interval_ptr, 
                size_of::<c_int>() as size_t) 
        };
 
        if ret == -1 {
            return Err(last_nano_error());
        }

        Ok(())
    }

    #[unstable]
    pub fn set_send_priority(&mut self, priority: u8) -> NanoResult<()> {
        let c_priority = priority as c_int;
        let c_priority_ptr = &c_priority as *const _ as *const c_void;
        let ret = unsafe { 
            libnanomsg::nn_setsockopt (
                self.socket, 
                libnanomsg::NN_SOL_SOCKET, 
                libnanomsg::NN_SNDPRIO, 
                c_priority_ptr, 
                size_of::<c_int>() as size_t) 
        };
 
        if ret == -1 {
            return Err(last_nano_error());
        }

        Ok(())
    }

    #[unstable]
    pub fn set_receive_priority(&mut self, priority: u8) -> NanoResult<()> {
        let c_priority = priority as c_int;
        let c_priority_ptr = &c_priority as *const _ as *const c_void;
        let ret = unsafe { 
            libnanomsg::nn_setsockopt (
                self.socket, 
                libnanomsg::NN_SOL_SOCKET, 
                libnanomsg::NN_RCVPRIO, 
                c_priority_ptr, 
                size_of::<c_int>() as size_t) 
        };
 
        if ret == -1 {
            return Err(last_nano_error());
        }

        Ok(())
    }

    #[unstable]
    pub fn set_ipv4_only(&mut self, ipv4_only: bool) -> NanoResult<()> {
        let c_ipv4_only = if ipv4_only { 1 as c_int } else { 0 as c_int };
        let option_value_ptr = &c_ipv4_only as *const _ as *const c_void;
        let ret = unsafe { 
            libnanomsg::nn_setsockopt (
                self.socket, 
                libnanomsg::NN_SOL_SOCKET, 
                libnanomsg::NN_IPV4ONLY, 
                option_value_ptr, 
                size_of::<c_int>() as size_t) 
        };
 
        if ret == -1 {
            return Err(last_nano_error());
        }

        Ok(())
    }
    
    #[unstable]
    pub fn set_socket_name(&mut self, name: &str) -> NanoResult<()> {
        let name_len = name.len() as size_t;
        let name_c_str = name.to_c_str();
        let name_ptr = name_c_str.as_ptr();
        let name_raw_ptr = name_ptr as *const c_void;
        let ret = unsafe { 
            libnanomsg::nn_setsockopt (
                self.socket, 
                libnanomsg::NN_SOL_SOCKET, 
                libnanomsg::NN_SOCKET_NAME, 
                name_raw_ptr, 
                name_len) 
        };
 
        if ret == -1 {
            return Err(last_nano_error());
        }

        Ok(())
    }

    #[unstable]
    pub fn set_tcp_nodelay(&mut self, tcp_nodelay: bool) -> NanoResult<()> {
        let c_tcp_nodelay = if tcp_nodelay { 1 as c_int } else { 0 as c_int };
        let option_value_ptr = &c_tcp_nodelay as *const _ as *const c_void;
        let ret = unsafe { 
            libnanomsg::nn_setsockopt (
                self.socket, 
                libnanomsg::NN_TCP, 
                libnanomsg::NN_TCP_NODELAY, 
                option_value_ptr, 
                size_of::<c_int>() as size_t) 
        };
 
        if ret == -1 {
            return Err(last_nano_error());
        }

        Ok(())
    }
    
    #[unstable]
    pub fn subscribe(&mut self, topic: &str) -> NanoResult<()> {
        let topic_len = topic.len() as size_t;
        let topic_c_str = topic.to_c_str();
        let topic_ptr = topic_c_str.as_ptr();
        let topic_raw_ptr = topic_ptr as *const c_void;
        let ret = unsafe { 
            libnanomsg::nn_setsockopt (self.socket, libnanomsg::NN_SUB, libnanomsg::NN_SUB_SUBSCRIBE, topic_raw_ptr, topic_len) 
        };
 
        if ret == -1 {
            return Err(last_nano_error());
        }

        Ok(())
    }

    #[unstable]
    pub fn unsubscribe(&mut self, topic: &str) -> NanoResult<()> {
        let topic_len = topic.len() as size_t;
        let topic_c_str = topic.to_c_str();
        let topic_ptr = topic_c_str.as_ptr();
        let topic_raw_ptr = topic_ptr as *const c_void;
        let ret = unsafe { 
            libnanomsg::nn_setsockopt (self.socket, libnanomsg::NN_SUB, libnanomsg::NN_SUB_UNSUBSCRIBE, topic_raw_ptr, topic_len) 
        };
 
        if ret == -1 {
            return Err(last_nano_error());
        }

        Ok(())
    }

    #[unstable]
    pub fn set_survey_deadline(&mut self, deadline: &Duration) -> NanoResult<()> {
        let milliseconds = deadline.num_milliseconds();
        let c_deadline = milliseconds as c_int;
        let c_deadline_ptr = &c_deadline as *const _ as *const c_void;
        let ret = unsafe { 
            libnanomsg::nn_setsockopt (
                self.socket, 
                libnanomsg::NN_SURVEYOR, 
                libnanomsg::NN_SURVEYOR_DEADLINE, 
                c_deadline_ptr, 
                size_of::<c_int>() as size_t) 
        };
 
        if ret == -1 {
            return Err(last_nano_error());
        }

        Ok(())
    }

    #[unstable]
    pub fn set_request_resend_interval(&mut self, interval: &Duration) -> NanoResult<()> {
        let milliseconds = interval.num_milliseconds();
        let c_interval = milliseconds as c_int;
        let c_interval_ptr = &c_interval as *const _ as *const c_void;
        let ret = unsafe { 
            libnanomsg::nn_setsockopt (
                self.socket, 
                libnanomsg::NN_REQ, 
                libnanomsg::NN_REQ_RESEND_IVL, 
                c_interval_ptr, 
                size_of::<c_int>() as size_t) 
        };
 
        if ret == -1 {
            return Err(last_nano_error());
        }

        Ok(())
    }

}

impl<'a> Reader for Socket<'a> {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {

        let buf_len = buf.len() as size_t;
        let buf_ptr = buf.as_mut_ptr();
        let c_buf_ptr = buf_ptr as *mut c_void;
        let ret = unsafe { libnanomsg::nn_recv(self.socket, c_buf_ptr, buf_len, 0 as c_int) };

        if ret == -1 {
            return Err(io::standard_error(io::OtherIoError));
        }

        Ok(ret as uint)
    }

    fn read_to_end(&mut self) -> IoResult<Vec<u8>> {
        let mut mem : *mut u8 = ptr::null_mut();

        let ret = unsafe {
            libnanomsg::nn_recv(
                self.socket, 
                transmute(&mut mem),
                libnanomsg::NN_MSG, 
                0 as c_int)
        };

        if ret == -1 {
            return Err(io::standard_error(io::OtherIoError));
        }

        let len = ret as uint;
        let c_bytes = unsafe { CVec::new(mem, len) };
        let mut bytes: Vec<u8> = Vec::with_capacity(len);

        bytes.push_all(c_bytes.as_slice());

        unsafe { libnanomsg::nn_freemsg(mem as *mut c_void) };

        Ok(bytes)
    }

    fn read_at_least(&mut self, min: uint, buf: &mut [u8]) -> IoResult<uint> {
        if min > buf.len() {
            return Err(io::standard_error(io::InvalidInput));
        }
        let mut read = 0;
        while read < min {
            loop {
                let write_buf = buf[mut read..];
                match self.read(write_buf) {
                    Ok(n) => {
                        read += std::cmp::min(n, write_buf.len());
                        break;
                    }
                    err@Err(_) => return err
                }
            }
        }
        Ok(read)
    }
}

impl<'a> Writer for Socket<'a> {
    fn write(&mut self, buf: &[u8]) -> IoResult<()> {
        let len = buf.len();
        let ret = unsafe {
            libnanomsg::nn_send(self.socket, buf.as_ptr() as *const c_void,
                                len as size_t, 0)
        };

        if ret as uint != len {
            return Err(io::standard_error(io::OtherIoError));
        }

        Ok(())
    }
}

#[unsafe_destructor]
impl<'a> Drop for Socket<'a> {
    fn drop(&mut self) {
        unsafe { libnanomsg::nn_close(self.socket); }
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused_must_use)]
    #[phase(plugin, link)]
    extern crate log;
    extern crate libnanomsg;
    extern crate libc;

    use super::*;
    use super::Protocol::*;
    use super::result::NanoErrorKind::*;

    use std::time::duration::Duration;
    use std::io::timer::sleep;

    use std::sync::{Arc, Barrier};

    #[test]
    fn initialize_socket() {
        let socket = match Socket::new(Pull) {
            Ok(socket) => socket,
            Err(err) => panic!("{}", err)
        };

        assert!(socket.socket >= 0);

        drop(socket)
    }

    #[test]
    fn bind_socket() {
        let mut socket = match Socket::new(Pull) {
            Ok(socket) => socket,
            Err(err) => panic!("{}", err)
        };

        match socket.bind("ipc:///tmp/bind_socket.ipc") {
            Ok(_) => {},
            Err(err) => panic!("{}", err)
        }

        drop(socket);
    }

    #[test]
    fn bind_and_shutdown() {
        let mut socket = match Socket::new(Pull) {
            Ok(socket) => socket,
            Err(err) => panic!("{}", err)
        };

        let mut endpoint = match socket.bind("ipc:///tmp/bind_and_shutdown.ipc") {
            Ok(endpoint) => endpoint,
            Err(err) => panic!("{}", err)
        };

        endpoint.shutdown();

        drop(socket);
    }

    #[test]
    fn connect_and_shutdown() {
        let mut socket = match Socket::new(Push) {
            Ok(socket) => socket,
            Err(err) => panic!("{}", err)
        };

        let mut endpoint = match socket.connect("ipc:///tmp/bind_and_shutdown.ipc") {
            Ok(endpoint) => endpoint,
            Err(err) => panic!("{}", err)
        };

        endpoint.shutdown();

        drop(socket);
    }

    fn test_create_socket<'a>(protocol: Protocol) -> Socket<'a> {
        match Socket::new(protocol) {
            Ok(socket) => socket,
            Err(err) => panic!("{}", err)
        }
    }

    fn test_bind<'b, 'a: 'b>(socket: &mut Socket<'a>, addr: &str) -> Endpoint<'b> {
        match socket.bind(addr) {
            Ok(endpoint) => endpoint,
            Err(err) => panic!("{}", err)
        }    
    }

    fn test_connect<'b, 'a: 'b>(socket: &mut Socket<'a>, addr: &str) -> Endpoint<'b> {
        match socket.connect(addr) {
            Ok(endpoint) => endpoint,
            Err(err) => panic!("{}", err)
        }    
    }

    fn test_write(socket: &mut Socket, buf: &[u8]) {
        match socket.write(buf) {
            Ok(..) => {},
            Err(err) => panic!("Failed to write to the socket: {}", err)
        }
    }

    fn test_read(socket: &mut Socket, expected: &[u8]) {
        let mut buf = [0u8, ..6];
        match socket.read(&mut buf) {
            Ok(len) => {
                assert_eq!(len, 6);
                assert_eq!(buf.as_slice(), expected)
            },
            Err(err) => panic!("{}", err)
        }
    }

    fn test_read_to_string(socket: &mut Socket, expected: &str) {
        match socket.read_to_string() {
            Ok(text) => assert_eq!(text.as_slice(), expected),
            Err(err) => panic!("{}", err)
        }
    }    

    fn test_subscribe(socket: &mut Socket, topic: &str) {
        match socket.subscribe(topic) {
            Ok(_) => {},
            Err(err) => panic!("{}", err)
        }    
    }

    #[test]
    fn pipeline() {

        let url = "ipc:///tmp/pipeline.ipc";

        let mut push_socket = test_create_socket(Push);
        test_bind(&mut push_socket, url);

        let mut pull_socket = test_create_socket(Pull);
        test_connect(&mut pull_socket, url);

        sleep(Duration::milliseconds(10));

        test_write(&mut push_socket, b"foobar");
        test_read(&mut pull_socket, b"foobar");

        drop(pull_socket);
        drop(push_socket);
    }

    fn test_multithread_pipeline(url: &'static str) {

        // this is required to stop the test main task only when children tasks are done
        let finish_line = Arc::new(Barrier::new(3));
        let finish_line_pull = finish_line.clone();
        let finish_line_push = finish_line.clone();

        spawn(proc() {
            let mut push_socket = test_create_socket(Push);
            
            test_bind(&mut push_socket, url);
            test_write(&mut push_socket, b"foobar");

            finish_line_push.wait();
        });

        spawn(proc() {
            let mut pull_socket = test_create_socket(Pull);

            test_connect(&mut pull_socket, url);
            test_read(&mut pull_socket, b"foobar");

            finish_line_pull.wait();
        });

        finish_line.wait();
    }

    #[test]
    fn pipeline_mt1() {
        test_multithread_pipeline("ipc:///tmp/pipeline_mt1.ipc")
    }
    
    #[test]
    fn pipeline_mt2() {
        test_multithread_pipeline("ipc:///tmp/pipeline_mt2.ipc")
    }

    #[test]
    fn pair() {

        let url = "ipc:///tmp/pair.ipc";

        let mut left_socket = test_create_socket(Pair);
        test_bind(&mut left_socket, url);

        let mut right_socket = test_create_socket(Pair);
        test_connect(&mut right_socket, url);

        sleep(Duration::milliseconds(10));

        test_write(&mut right_socket, b"foobar");
        test_read(&mut left_socket, b"foobar");

        test_write(&mut left_socket, b"foobaz");
        test_read(&mut right_socket, b"foobaz");

        drop(left_socket);
        drop(right_socket);
    }

    #[test]
    fn bus() {
         
        let url = "ipc:///tmp/bus.ipc";

        let mut sock1 = test_create_socket(Bus);
        test_bind(&mut sock1, url);

        let mut sock2 = test_create_socket(Bus);
        test_connect(&mut sock2, url);

        let mut sock3 = test_create_socket(Bus);
        test_connect(&mut sock3, url);

        sleep(Duration::milliseconds(10));

        let msg = b"foobar";
        test_write(&mut sock1, msg);
        test_read(&mut sock2, msg);
        test_read(&mut sock3, msg);

        drop(sock3);
        drop(sock2);
        drop(sock1);
    }

    #[test]
    fn pubsub() {
         
        let url = "ipc:///tmp/pubsub.ipc";

        let mut sock1 = test_create_socket(Pub);
        test_bind(&mut sock1, url);

        let mut sock2 = test_create_socket(Sub);
        test_subscribe(&mut sock2, "foo");
        test_connect(&mut sock2, url);

        let mut sock3 = test_create_socket(Sub);
        test_subscribe(&mut sock3, "bar");
        test_connect(&mut sock3, url);

        sleep(Duration::milliseconds(10));

        let msg1 = b"foobar";
        test_write(&mut sock1, msg1);
        test_read(&mut sock2, msg1);

        let msg2 = b"barfoo";
        test_write(&mut sock1, msg2);
        test_read(&mut sock3, msg2);

        drop(sock3);
        drop(sock2);
        drop(sock1);
    }

    #[test]
    fn survey() {
         
        let url = "ipc:///tmp/survey.ipc";

        let mut sock1 = test_create_socket(Surveyor);
        test_bind(&mut sock1, url);

        let mut sock2 = test_create_socket(Respondent);
        test_connect(&mut sock2, url);

        let mut sock3 = test_create_socket(Respondent);
        test_connect(&mut sock3, url);

        sleep(Duration::milliseconds(10));

        let deadline = Duration::milliseconds(500);
        match sock1.set_survey_deadline(&deadline) {
            Ok(socket) => socket,
            Err(err) => panic!("{}", err)
        };

        let question = b"yesno?";
        test_write(&mut sock1, question);
        test_read(&mut sock2, question);
        test_read(&mut sock3, question);

        let vote = b"may be";
        test_write(&mut sock2, vote);
        test_write(&mut sock3, vote);
        test_read(&mut sock1, vote);
        test_read(&mut sock1, vote);

        drop(sock3);
        drop(sock2);
        drop(sock1);
    }

    #[test]
    fn should_change_linger() {

        let mut socket = test_create_socket(Pair);

        let linger = Duration::milliseconds(1024);
        match socket.set_linger(&linger) {
            Ok(..) => {},
            Err(err) => panic!("Failed to change linger on the socket: {}", err)
        }

        drop(socket)
    }

    #[test]
    fn should_change_send_buffer_size() {

        let mut socket = test_create_socket(Pair);

        let size: int = 64 * 1024;
        match socket.set_send_buffer_size(size) {
            Ok(..) => {},
            Err(err) => panic!("Failed to change send buffer size on the socket: {}", err)
        }

        drop(socket)
    }

    #[test]
    fn should_change_receive_buffer_size() {

        let mut socket = test_create_socket(Pair);

        let size: int = 64 * 1024;
        match socket.set_receive_buffer_size(size) {
            Ok(..) => {},
            Err(err) => panic!("Failed to change receive buffer size on the socket: {}", err)
        }

        drop(socket)
    }

    #[test]
    fn should_change_send_timeout() {

        let mut socket = test_create_socket(Pair);

        let timeout = Duration::milliseconds(-2);
        match socket.set_send_timeout(&timeout) {
            Ok(..) => {},
            Err(err) => panic!("Failed to change send timeout on the socket: {}", err)
        }

        drop(socket)
    }

    #[test]
    fn should_change_receive_timeout() {

        let mut socket = test_create_socket(Pair);

        let timeout = Duration::milliseconds(200);
        match socket.set_receive_timeout(&timeout) {
            Ok(..) => {},
            Err(err) => panic!("Failed to change receive timeout on the socket: {}", err)
        }

        drop(socket)
    }

    #[test]
    fn should_change_reconnect_interval() {

        let mut socket = test_create_socket(Pair);

        let interval = Duration::milliseconds(142);
        match socket.set_reconnect_interval(&interval) {
            Ok(..) => {},
            Err(err) => panic!("Failed to change reconnect interval on the socket: {}", err)
        }

        drop(socket)
    }

    #[test]
    fn should_change_max_reconnect_interval() {

        let mut socket = test_create_socket(Pair);

        let interval = Duration::milliseconds(666);
        match socket.set_max_reconnect_interval(&interval) {
            Ok(..) => {},
            Err(err) => panic!("Failed to change reconnect interval on the socket: {}", err)
        }

        drop(socket)
    }

    #[test]
    fn should_change_send_priority() {

        let mut socket = test_create_socket(Pair);

        match socket.set_send_priority(15u8) {
            Ok(..) => {},
            Err(err) => panic!("Failed to change send priority on the socket: {}", err)
        }

        drop(socket)
    }

    #[test]
    fn should_change_receive_priority() {

        let mut socket = test_create_socket(Pair);

        match socket.set_receive_priority(2u8) {
            Ok(..) => {},
            Err(err) => panic!("Failed to change receive priority on the socket: {}", err)
        }

        drop(socket)
    }

    #[test]
    fn should_change_ipv4_only() {

        let mut socket = test_create_socket(Pair);

        match socket.set_ipv4_only(true) {
            Ok(..) => {},
            Err(err) => panic!("Failed to change ipv4 only on the socket: {}", err)
        }

        drop(socket)
    }

    #[test]
    fn should_change_socket_name() {

        let mut socket = test_create_socket(Pair);

        match socket.set_socket_name("bob") {
            Ok(..) => {},
            Err(err) => panic!("Failed to change the socket name: {}", err)
        }

        drop(socket)
    }

    #[test]
    fn should_change_request_resend_interval() {

        let mut socket = test_create_socket(Req);

        let interval = Duration::milliseconds(60042);
        match socket.set_request_resend_interval(&interval) {
            Ok(..) => {},
            Err(err) => panic!("Failed to change request resend interval on the socket: {}", err)
        }

        drop(socket)
    }

    #[test]
    fn should_change_tcp_nodelay() {

        let mut socket = test_create_socket(Pair);

        match socket.set_tcp_nodelay(true) {
            Ok(..) => {},
            Err(err) => panic!("Failed to change tcp nodelay only on the socket: {}", err)
        }

        drop(socket)
    }

    #[test]
    fn protcol_matches_raw() {
         assert_eq!(libnanomsg::NN_REQ, Req.to_raw())
         assert_eq!(libnanomsg::NN_REP, Rep.to_raw())
         assert_eq!(libnanomsg::NN_PUSH, Push.to_raw())
         assert_eq!(libnanomsg::NN_PULL, Pull.to_raw())
         assert_eq!(libnanomsg::NN_PAIR, Pair.to_raw())
         assert_eq!(libnanomsg::NN_BUS, Bus.to_raw())
         assert_eq!(libnanomsg::NN_PUB, Pub.to_raw())
         assert_eq!(libnanomsg::NN_SUB, Sub.to_raw())
         assert_eq!(libnanomsg::NN_SURVEYOR, Surveyor.to_raw())
         assert_eq!(libnanomsg::NN_RESPONDENT, Respondent.to_raw())
    }

    #[test]
    fn test_read_to_end() {

        let url = "ipc:///tmp/read_to_end.ipc";

        let mut left_socket = test_create_socket(Pair);
        test_bind(&mut left_socket, url);

        let mut right_socket = test_create_socket(Pair);
        test_connect(&mut right_socket, url);

        sleep(Duration::milliseconds(10));

        test_write(&mut right_socket, b"ok");
        test_read_to_string(&mut left_socket, "ok".as_slice());

        test_write(&mut left_socket, b"");
        test_read_to_string(&mut right_socket, "".as_slice());

        test_write(&mut left_socket, b"not ok");
        test_read_to_string(&mut right_socket, "not ok".as_slice());

        drop(left_socket);
        drop(right_socket);
    }

    #[test]
    fn nb_read_works_in_both_cases() {

        let url = "ipc:///tmp/nb_read_works_in_both_cases.ipc";

        let mut push_socket = test_create_socket(Push);
        test_bind(&mut push_socket, url);

        let mut pull_socket = test_create_socket(Pull);
        test_connect(&mut pull_socket, url);
        sleep(Duration::milliseconds(10));

        let mut buf = [0u8, ..6];
        match pull_socket.nb_read(&mut buf) {
            Ok(_) => panic!("Nothing should have been received !"),
            Err(err) => assert_eq!(err.kind, TryAgain)
        }

        test_write(&mut push_socket, b"foobar");
        sleep(Duration::milliseconds(10));

        let mut buf = [0u8, ..6];
        match pull_socket.nb_read(&mut buf) {
            Ok(len) => {
                assert_eq!(len, 6);
                assert_eq!(buf.as_slice(), b"foobar")
            },
            Err(err) => panic!("{}", err)
        }

        drop(pull_socket);
        drop(push_socket);
    }

    #[test]
    fn nb_read_to_end_works_in_both_cases() {

        let url = "ipc:///tmp/nb_read_to_end_works_in_both_cases.ipc";

        let mut push_socket = test_create_socket(Push);
        test_bind(&mut push_socket, url);

        let mut pull_socket = test_create_socket(Pull);
        test_connect(&mut pull_socket, url);
        sleep(Duration::milliseconds(10));

        match pull_socket.nb_read_to_end() {
            Ok(_) => panic!("Nothing should have been received !"),
            Err(err) => assert_eq!(err.kind, TryAgain)
        }

        test_write(&mut push_socket, b"foobar");
        sleep(Duration::milliseconds(10));

        match pull_socket.nb_read_to_end() {
            Ok(buf) => {
                assert_eq!(buf.len(), 6);
                assert_eq!(buf.as_slice(), b"foobar")
            },
            Err(err) => panic!("{}", err)
        }

        drop(pull_socket);
        drop(push_socket);
    }

    #[test]
    fn nb_write_works_in_both_cases() {

        let url = "ipc:///tmp/nb_write_works_in_both_cases.ipc";

        let mut push_socket = test_create_socket(Push);
        test_bind(&mut push_socket, url);
        sleep(Duration::milliseconds(10));

        match push_socket.nb_write(b"barfoo") {
            Ok(_) => panic!("Nothing should have been sent !"),
            Err(err) => assert_eq!(err.kind, TryAgain)
        }

        drop(push_socket);
    }

    #[test]
    fn poll_works() {
        let url = "ipc:///tmp/poll_works_.ipc";

        let mut left_socket = test_create_socket(Pair);
        test_bind(&mut left_socket, url);

        let mut right_socket = test_create_socket(Pair);
        test_connect(&mut right_socket, url);

        sleep(Duration::milliseconds(10));

        let pollfd1 = left_socket.new_pollfd(true, true);
        let pollfd2 = right_socket.new_pollfd(true, true);

        // TODO : find some simpler/shorter/better way to intialize a poll request
        let mut pollreq_vector: Vec<PollFd> = vec![pollfd1, pollfd2];
        let mut pollreq_slice = pollreq_vector.as_mut_slice();
        let mut request = PollRequest::new(pollreq_slice);
        let timeout = Duration::milliseconds(10);
        {
            let poll_result = Socket::poll(&mut request, &timeout);
        
            match poll_result {
                Ok(count) => assert_eq!(2, count),
                Err(err) => panic!("Failed to poll: {}", err)
            }
    
            let fds = request.get_fds();
            assert_eq!(true, fds[0].can_write());
            assert_eq!(false, fds[0].can_read());
            assert_eq!(true, fds[1].can_write());
            assert_eq!(false, fds[1].can_read());
        }

        test_write(&mut right_socket, b"foobar");
        sleep(Duration::milliseconds(10));
        {
            let poll_result = Socket::poll(&mut request, &timeout);
        
            match poll_result {
                Ok(count) => assert_eq!(2, count),
                Err(err) => panic!("Failed to poll: {}", err)
            }
    
            let fds = request.get_fds();
            assert_eq!(true, fds[0].can_write());
            assert_eq!(true, fds[0].can_read()); // and now right socket can read the msg sent by left
            assert_eq!(true, fds[1].can_write());
            assert_eq!(false, fds[1].can_read());
        }
    }
}
