use std::fmt;
use std::io;
use std::mem;
use std::net::Shutdown;
use std::os::raw::c_int;
use std::os::windows::io::{AsRawSocket, FromRawSocket, IntoRawSocket, RawSocket};
use std::path::Path;

use ws2_32::{bind, connect, getpeername, getsockname, listen};

use super::socket::{init, Socket};
use super::{c, cvt, from_sockaddr_un, sockaddr_un, SocketAddr};

/// A Unix stream socket
pub struct UnixStream(Socket);

impl fmt::Debug for UnixStream {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let mut builder = fmt.debug_struct("UnixStream");
        builder.field("socket", &self.0.as_raw_socket());
        if let Ok(addr) = self.local_addr() {
            builder.field("local", &addr);
        }
        if let Ok(addr) = self.peer_addr() {
            builder.field("peer", &addr);
        }
        builder.finish()
    }
}

impl UnixStream {
    /// Connects to the socket named by `path`.
    pub fn connect<P: AsRef<Path>>(path: P) -> io::Result<UnixStream> {
        init();
        fn inner(path: &Path) -> io::Result<UnixStream> {
            unsafe {
                let inner = Socket::new()?;
                let (addr, len) = sockaddr_un(path)?;

                cvt(connect(
                    inner.as_raw_socket() as _,
                    &addr as *const _ as *const _,
                    len as i32,
                ))?;
                Ok(UnixStream(inner))
            }
        }
        inner(path.as_ref())
    }

    /// Creates a new independently owned handle to the underlying socket.
    ///
    /// The returned `UnixStream` is a reference to the same stream that this
    /// object references. Both handles will read and write the same stream of
    /// data, and options set on one stream will be propagated to the other
    /// stream.
    pub fn try_clone(&self) -> io::Result<UnixStream> {
        self.0.duplicate().map(UnixStream)
    }

    /// Returns the socket address of the local half of this connection.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        SocketAddr::new(|addr, len| unsafe { getsockname(self.0.as_raw_socket() as _, addr, len) })
    }

    /// Returns the socket address of the remote half of this connection.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        SocketAddr::new(|addr, len| unsafe { getpeername(self.0.as_raw_socket() as _, addr, len) })
    }

    /// Moves the socket into or out of nonblocking mode.
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }

    /// Returns the value of the `SO_ERROR` option.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.0.take_error()
    }

    /// Shuts down the read, write, or both halves of this connection.
    ///
    /// This function will cause all pending and future I/O calls on the
    /// specified portions to immediately return with an appropriate value
    /// (see the documentation for `Shutdown`).
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.0.shutdown(how)
    }
}

impl io::Read for UnixStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        io::Read::read(&mut &*self, buf)
    }
}

impl<'a> io::Read for &'a UnixStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl io::Write for UnixStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        io::Write::write(&mut &*self, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::Write::flush(&mut &*self)
    }
}

impl<'a> io::Write for &'a UnixStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsRawSocket for UnixStream {
    fn as_raw_socket(&self) -> RawSocket {
        self.0.as_raw_socket()
    }
}

impl FromRawSocket for UnixStream {
    unsafe fn from_raw_socket(sock: RawSocket) -> Self {
        UnixStream(Socket::from_raw_socket(sock))
    }
}

impl IntoRawSocket for UnixStream {
    fn into_raw_socket(self) -> RawSocket {
        let ret = self.0.as_raw_socket();
        mem::forget(self);
        ret
    }
}

/// A Unix domain socket server
pub struct UnixListener(Socket);

impl fmt::Debug for UnixListener {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let mut builder = fmt.debug_struct("UnixListener");
        builder.field("socket", &self.0.as_raw_socket());
        if let Ok(addr) = self.local_addr() {
            builder.field("local", &addr);
        }
        builder.finish()
    }
}

impl UnixListener {
    /// Creates a new `UnixListener` bound to the specified socket.
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<UnixListener> {
        init();
        fn inner(path: &Path) -> io::Result<UnixListener> {
            unsafe {
                let inner = Socket::new()?;
                let (addr, len) = sockaddr_un(path)?;

                cvt(bind(
                    inner.as_raw_socket() as _,
                    &addr as *const _ as *const _,
                    len as _,
                ))?;
                cvt(listen(inner.as_raw_socket() as _, 128))?;

                Ok(UnixListener(inner))
            }
        }
        inner(path.as_ref())
    }

    /// Accepts a new incoming connection to this listener.
    ///
    /// This function will block the calling thread until a new Unix connection
    /// is established. When established, the corresponding [`UnixStream`] and
    /// the remote peer's address will be returned.
    pub fn accept(&self) -> io::Result<(UnixStream, SocketAddr)> {
        let mut storage: c::sockaddr_un = unsafe { mem::zeroed() };
        let mut len = mem::size_of_val(&storage) as c_int;
        let sock = self.0.accept(&mut storage as *mut _ as *mut _, &mut len)?;
        let addr = from_sockaddr_un(storage, len)?;
        Ok((UnixStream(sock), addr))
    }

    /// Creates a new independently owned handle to the underlying socket.
    ///
    /// The returned `UnixListener` is a reference to the same socket that this
    /// object references. Both handles can be used to accept incoming
    /// connections and options set on one listener will affect the other.
    pub fn try_clone(&self) -> io::Result<UnixListener> {
        self.0.duplicate().map(UnixListener)
    }

    /// Returns the local socket address of this listener.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        SocketAddr::new(|addr, len| unsafe { getsockname(self.0.as_raw_socket() as _, addr, len) })
    }

    /// Moves the socket into or out of nonblocking mode.
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }

    /// Returns the value of the `SO_ERROR` option.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.0.take_error()
    }

    /// Returns an iterator over incoming connections.
    ///
    /// The iterator will never return `None` and will also not yield the
    /// peer's [`SocketAddr`] structure.
    ///
    /// [`SocketAddr`]: struct.SocketAddr.html
    pub fn incoming<'a>(&'a self) -> Incoming<'a> {
        Incoming { listener: self }
    }
}

impl AsRawSocket for UnixListener {
    fn as_raw_socket(&self) -> RawSocket {
        self.0.as_raw_socket()
    }
}

impl FromRawSocket for UnixListener {
    unsafe fn from_raw_socket(sock: RawSocket) -> Self {
        UnixListener(Socket::from_raw_socket(sock))
    }
}

impl IntoRawSocket for UnixListener {
    fn into_raw_socket(self) -> RawSocket {
        let ret = self.0.as_raw_socket();
        mem::forget(self);
        ret
    }
}

impl<'a> IntoIterator for &'a UnixListener {
    type Item = io::Result<UnixStream>;
    type IntoIter = Incoming<'a>;

    fn into_iter(self) -> Incoming<'a> {
        self.incoming()
    }
}

/// An iterator over incoming connections to a [`UnixListener`].
///
/// It will never return `None`.
///
/// [`UnixListener`]: struct.UnixListener.html
#[derive(Debug)]
pub struct Incoming<'a> {
    listener: &'a UnixListener,
}

impl<'a> Iterator for Incoming<'a> {
    type Item = io::Result<UnixStream>;

    fn next(&mut self) -> Option<io::Result<UnixStream>> {
        Some(self.listener.accept().map(|s| s.0))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::max_value(), None)
    }
}

#[cfg(test)]
mod test {
    extern crate tempdir;

    use std::io::{self, Read, Write};
    use std::path::PathBuf;
    use std::thread;

    use self::tempdir::TempDir;

    use super::*;

    macro_rules! or_panic {
        ($e:expr) => {
            match $e {
                Ok(e) => e,
                Err(e) => panic!("{}", e),
            }
        };
    }

    fn tmpdir() -> Result<(TempDir, PathBuf), io::Error> {
        let dir = TempDir::new("mio-uds-windows")?;
        let path = dir.path().join("sock");
        Ok((dir, path))
    }

    #[test]
    fn basic() {
        let (_dir, socket_path) = or_panic!(tmpdir());
        let msg1 = b"hello";
        let msg2 = b"world!";

        let listener = or_panic!(UnixListener::bind(&socket_path));
        let thread = thread::spawn(move || {
            let mut stream = or_panic!(listener.accept()).0;
            let mut buf = [0; 5];
            or_panic!(stream.read(&mut buf));
            assert_eq!(&msg1[..], &buf[..]);
            or_panic!(stream.write_all(msg2));
        });

        let mut stream = or_panic!(UnixStream::connect(&socket_path));
        assert_eq!(
            Some(&*socket_path),
            stream.peer_addr().unwrap().as_pathname()
        );
        or_panic!(stream.write_all(msg1));
        let mut buf = vec![];
        or_panic!(stream.read_to_end(&mut buf));
        assert_eq!(&msg2[..], &buf[..]);
        drop(stream);

        thread.join().unwrap();
    }

    #[test]
    fn try_clone() {
        let (_dir, socket_path) = or_panic!(tmpdir());
        let msg1 = b"hello";
        let msg2 = b"world";

        let listener = or_panic!(UnixListener::bind(&socket_path));
        let thread = thread::spawn(move || {
            #[allow(unused_mut)]
            let mut stream = or_panic!(listener.accept()).0;
            or_panic!(stream.write_all(msg1));
            or_panic!(stream.write_all(msg2));
        });

        let mut stream = or_panic!(UnixStream::connect(&socket_path));
        let mut stream2 = or_panic!(stream.try_clone());
        assert_eq!(
            Some(&*socket_path),
            stream2.peer_addr().unwrap().as_pathname()
        );

        let mut buf = [0; 5];
        or_panic!(stream.read(&mut buf));
        assert_eq!(&msg1[..], &buf[..]);
        or_panic!(stream2.read(&mut buf));
        assert_eq!(&msg2[..], &buf[..]);

        thread.join().unwrap();
    }

    #[test]
    fn iter() {
        let (_dir, socket_path) = or_panic!(tmpdir());

        let listener = or_panic!(UnixListener::bind(&socket_path));
        let thread = thread::spawn(move || {
            for stream in listener.incoming().take(2) {
                let mut stream = or_panic!(stream);
                let mut buf = [0];
                or_panic!(stream.read(&mut buf));
            }
        });

        for _ in 0..2 {
            let mut stream = or_panic!(UnixStream::connect(&socket_path));
            or_panic!(stream.write_all(&[0]));
        }

        thread.join().unwrap();
    }

    #[test]
    fn long_path() {
        let dir = or_panic!(TempDir::new("mio-uds-windows"));
        let socket_path = dir.path().join(
            "asdfasdfasdfasdfasdfasdfasdfasdfasdfasdfasdfasdfasdfasdfasdfa\
                                    sasdfasdfasdasdfasdfasdfadfasdfasdfasdfasdfasdf",
        );
        match UnixStream::connect(&socket_path) {
            Err(ref e) if e.kind() == io::ErrorKind::InvalidInput => {}
            Err(e) => panic!("unexpected error {}", e),
            Ok(_) => panic!("unexpected success"),
        }

        match UnixListener::bind(&socket_path) {
            Err(ref e) if e.kind() == io::ErrorKind::InvalidInput => {}
            Err(e) => panic!("unexpected error {}", e),
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn abstract_namespace_not_allowed() {
        assert!(UnixStream::connect("\0asdf").is_err());
    }
}
