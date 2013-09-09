// ======================================
// nanomsg.rs : nanomsg bindings for rust
//
// This aims to be a rust version of the 
// full public API of nanomsg. But parts
// are probably still missing, since the
// safe API only does nn_send and nn_recv currently.
// ======================================

extern mod std;

use std::libc::*;
use std::ptr;
use std::unstable::intrinsics;
use std::cast::transmute;
use std::option::Option;
use std::num::*;
use std::os::*;

pub static AF_SP: c_int = 1;
pub static AF_SP_RAW: c_int = 2;
pub static NN_CHUNKREF_MAX: c_int = 32;
pub static NN_DOMAIN: c_int = 12;
pub static NN_DONTWAIT: c_int = 1;
pub static NN_FSM_ACTION: c_int = -2;
pub static NN_FSM_START: c_int = -2;
pub static NN_FSM_STOP: c_int = -3;
pub static NN_HAUSNUMERO: c_int = 156384712;
pub static NN_INPROC: c_int = -1;
pub static NN_IPC: c_int = -2;
pub static NN_IPV4ONLY: c_int = 14;
pub static NN_LINGER: c_int = 1;
pub static NN_PIPEBASE_PARSED: c_int = 2;
pub static NN_PIPEBASE_RELEASE: c_int = 1;
pub static NN_PIPE_IN: c_int = 33987;
pub static NN_PIPE_OUT: c_int = 33988;
pub static NN_PIPE_PARSED: c_int = 2;
pub static NN_PIPE_RELEASE: c_int = 1;
pub static NN_PROTO_BUS: c_int = 7;
pub static NN_PROTOCOL: c_int = 13;
pub static NN_PROTO_PAIR: c_int = 1;
pub static NN_PROTO_PIPELINE: c_int = 5;
pub static NN_PROTO_PUBSUB: c_int = 2;
pub static NN_PROTO_REQREP: c_int = 3;
pub static NN_PROTO_SURVEY: c_int = 6;
pub static NN_RCVBUF: c_int = 3;
pub static NN_RCVFD: c_int = 11;
pub static NN_RCVTIMEO: c_int = 5;
pub static NN_RECONNECT_IVL: c_int = 6;
pub static NN_RECONNECT_IVL_MAX: c_int = 7;
pub static NN_REQ_RESEND_IVL: c_int = 1;
pub static NN_SNDBUF: c_int = 2;
pub static NN_SNDFD: c_int = 10;
pub static NN_SNDPRIO: c_int = 8;
pub static NN_SNDTIMEO: c_int = 4;
pub static NN_SOCKADDR_MAX: c_int = 128;
pub static NN_SOCKBASE_EVENT_IN: c_int = 1;
pub static NN_SOCKBASE_EVENT_OUT: c_int = 2;
pub static NN_SOCKTYPE_FLAG_NORECV: c_int = 1;
pub static NN_SOCKTYPE_FLAG_NOSEND: c_int = 2;
pub static NN_SOL_SOCKET: c_int = 0;
pub static NN_SUB_SUBSCRIBE: c_int = 1;
pub static NN_SUB_UNSUBSCRIBE: c_int = 2;
pub static NN_SURVEYOR_DEADLINE: c_int = 1;
pub static NN_TCP: c_int = -3;
pub static NN_TCP_NODELAY: c_int = 1;
pub static NN_VERSION_AGE: c_int = 0;
pub static NN_VERSION_CURRENT: c_int = 0;
pub static NN_VERSION_REVISION: c_int = 0;

pub static NN_BUS: c_int = (NN_PROTO_BUS * 16 + 0);
pub static NN_MSG: u64 = -1;
pub static NN_PAIR: c_int = (NN_PROTO_PAIR * 16 + 0);
pub static NN_PUSH: c_int = (NN_PROTO_PIPELINE * 16 + 0);
pub static NN_PULL: c_int = (NN_PROTO_PIPELINE * 16 + 1);
pub static NN_PUB: c_int = (NN_PROTO_PUBSUB * 16 + 0);
pub static NN_SUB: c_int = (NN_PROTO_PUBSUB * 16 + 1);
pub static NN_REQ: c_int = (NN_PROTO_REQREP * 16 + 0);
pub static NN_REP: c_int = (NN_PROTO_REQREP * 16 + 1);
pub static NN_SURVEYOR: c_int = (NN_PROTO_SURVEY * 16 + 0);
pub static NN_RESPONDENT: c_int = (NN_PROTO_SURVEY * 16 + 1);
pub static EACCESS: c_int = (NN_HAUSNUMERO + 17);
pub static ETERM: c_int = (NN_HAUSNUMERO + 53);
pub static EFSM: c_int = (NN_HAUSNUMERO + 54);
pub static NN_QUEUE_NOTINQUEUE: c_int = -1;
pub static NN_LIST_NOTINLIST: c_int = -1;

pub type error_t = c_int;
pub type ptrdiff_t = c_long;
pub type size_t = c_ulong;
pub type wchar_t = c_int;

pub struct Struct_nn_iovec {
    iov_base: *mut c_void,
    iov_len: size_t,
}

pub struct Struct_nn_msghdr {
    msg_iov: *mut Struct_nn_iovec,
    msg_iovlen: c_int,
    msg_control: *mut c_void,
    msg_controllen: size_t,
}

pub struct Struct_nn_cmsghdr {
    cmsg_len: size_t,
    cmsg_level: c_int,
    cmsg_type: c_int,
}

#[link_args = "-lnanomsg"]
#[fixed_stack_segment]
extern "C" {
    pub static mut program_invocation_name: *mut c_schar;

    pub static mut program_invocation_short_name: *mut c_schar;

    pub fn __errno_location() -> *mut c_int;

    pub fn nn_errno() -> c_int;

    pub fn nn_strerror(errnum: c_int) -> *c_schar;

    pub fn nn_symbol(i: c_int, 
                     value: *mut c_int) -> *c_schar;

    pub fn nn_term();

    pub fn nn_allocmsg(size: size_t, 
                       _type: c_int) -> *mut c_void;

    pub fn nn_freemsg(msg: *mut c_void) -> c_int;

    pub fn nn_socket(domain: c_int, protocol: c_int) -> c_int;

    pub fn nn_close(s: c_int) -> c_int;

    pub fn nn_setsockopt(s: c_int, 
                         level: c_int, 
                         option: c_int,
                         optval: *c_void, 
                         optvallen: size_t) -> c_int;

    pub fn nn_getsockopt(s: c_int, level: c_int, 
                         option: c_int,
                         optval: *mut c_void, 
                         optvallen: *mut size_t) -> c_int;

    pub fn nn_bind(s: c_int, addr: *c_schar) -> c_int;

    pub fn nn_connect(s: c_int, addr: *c_schar) -> c_int;

    pub fn nn_shutdown(s: c_int, how: c_int) -> c_int;

    pub fn nn_send(s: c_int, 
                   buf: *c_void, 
                   len: size_t, 
                   flags: c_int) -> c_int;
    
    pub fn nn_recv(s: c_int, 
                   buf: *mut c_void, 
                   len: size_t, 
                   flags: c_int) -> c_int;

    pub fn nn_sendmsg(s: c_int, 
                      msghdr: *Struct_nn_msghdr, 
                      flags: c_int) -> c_int;

    pub fn nn_recvmsg(s: c_int, 
                      msghdr: *mut Struct_nn_msghdr, 
                      flags: c_int) -> c_int;

    pub fn nn_device(s1: c_int, 
                     s2: c_int) -> c_int;
}


enum HowToCleanup {
  Free,
  Call_nn_freemsg,
  DoNothing
}

// a wrapper around the message returned by nn_recv
pub struct NanoMsg {
    buf: *mut u8,
    bytes_stored_in_buf: u64,
    bytes_available: u64,
    priv cleanup: HowToCleanup,
}


impl NanoMsg {

    pub fn new() -> NanoMsg {
        let buf : *mut u8 = 0 as *mut u8;
        NanoMsg{buf: buf, bytes_stored_in_buf: 0, bytes_available: 0, cleanup: DoNothing }
    }

    pub fn len(&self) -> u64 {
        self.bytes_stored_in_buf
    }

    pub fn actual_msg_bytes_avail(&self) -> u64 {
        self.bytes_available
    }

    pub fn printbuf(&self) {
        printfln!("NanoMsg contains message of length %?: '%s'", self.bytes_stored_in_buf, self.copy_to_string());
    }

    /// Unwraps the NanoMsg.
    /// Any ownership of the message pointed to by buf is forgotten.
    pub unsafe fn unwrap(self) -> *mut u8 {
        printfln!("we should never get here!!!!");
        assert!(false);
        let mut msg = self;
        msg.cleanup = DoNothing;
        msg.buf
    }

    pub fn recv_any_size(&mut self, sock: c_int, flags: c_int) -> Option<u64> {
        #[fixed_stack_segment];
        #[inline(never)];

        match(self.cleanup) {
            DoNothing => (),
            Free => self.cleanup(),
            Call_nn_freemsg => self.cleanup()
        }
            
        unsafe { 
            self.bytes_stored_in_buf = nn_recv (sock,  transmute(&mut self.buf), NN_MSG, flags) as u64;
        }

        if (self.bytes_stored_in_buf < 0) {
            printfln!("nn_recv failed with errno: %? '%?'", std::os::errno(), std::os::last_os_error());
            return None;
        } else {
            self.cleanup = Call_nn_freemsg;
            return Some(self.bytes_stored_in_buf);
        }
    }


    // truncates any part of the message over maxlen
    pub fn recv_no_more_than_maxlen(&mut self, sock: c_int, maxlen: u64, flags: c_int) -> Option<u64> {
        #[fixed_stack_segment];
        #[inline(never)];

        match(self.cleanup) {
            DoNothing => (),
            Free => self.cleanup(),
            Call_nn_freemsg => self.cleanup()
        }

        unsafe { 
            let ptr = malloc(maxlen as size_t) as *mut u8;
            assert!(!ptr::is_null(ptr));
            self.cleanup = Free;

            self.buf = ptr;
            self.bytes_available = nn_recv (sock, 
                                           transmute(self.buf),
                                           maxlen, 
                                           flags) as u64;
            
            if (self.bytes_available < 0) {
                printfln!("recv_no_more_than_maxlen: nn_recv failed with errno: %? '%?'", std::os::errno(), std::os::last_os_error());
                return None;
            }

            if (self.bytes_available > maxlen) {
                let errmsg = fmt!("recv_no_more_than_maxlen: message was longer (%? bytes) than we allocated space for (%? bytes)", self.bytes_available, maxlen);
                printfln!(errmsg);
                warn!(errmsg);
            }

            self.bytes_stored_in_buf = min(maxlen, self.bytes_available);            
            Some(self.bytes_stored_in_buf)
        }
    }

    pub fn copy_to_string(&self) -> ~str {
        printfln!("copy to string sees size: '%?'", self.bytes_stored_in_buf);
        printfln!("copy to string sees buf : '%?'", self.buf as *u8);
        unsafe { std::str::raw::from_buf_len(self.buf as *u8, self.bytes_stored_in_buf as uint) }
    }

    pub fn cleanup(&self) {
        #[fixed_stack_segment];
        #[inline(never)];

        if (std::ptr::is_null(self.buf)) { return; }

        match self.cleanup {
            DoNothing => (),
            Free => {
                unsafe {
                    // see example code: http://static.rust-lang.org/doc/tutorial-ffi.html

                    let x = intrinsics::init(); // dummy value to swap in
                    // moving the object out is needed to call the destructor
                    ptr::replace_ptr(self.buf, x);
                    free(self.buf as *c_void)
                }
            },

            Call_nn_freemsg => {
                unsafe {
                    printfln!("*** Call_nn_freemsg Drop running.");

                    let x = intrinsics::init(); // dummy value to swap in
                    // moving the object out is needed to call the destructor
                    ptr::replace_ptr(self.buf, x);
                    
                    let rc = nn_freemsg(self.buf as *mut c_void);
                    assert! (rc == 0);
                }
            }
        }
        
    }
    
}

#[unsafe_destructor]
impl Drop for NanoMsg {
    fn drop(&self) {
        #[fixed_stack_segment];
        #[inline(never)];
        printfln!("starting Drop for NanoMsg, with style: %?", self.cleanup);
        self.cleanup();
    }
}

