use varnish::vcl::NativeBackend;

varnish::run_vtc_tests!("tests/*.vtc");

pub struct DynamicBackend {
    /// A native backend created alongside this `DynamicBackend`.
    backend: NativeBackend,
}

#[varnish::vmod(docs = "README.md")]
mod native_backend {
    use super::DynamicBackend;
    use std::net::SocketAddr;
    use std::ptr::null;
    use varnish::ffi;
    use varnish::vcl::Ctx;
    use varnish::vcl::IntoVCL;
    use varnish::vcl::NativeBackendBuilder;
    use varnish::vcl::Probe;
    use varnish::vcl::{BackendRef, NativeBackend};

    impl DynamicBackend {
        /// Create a backend with an optional probe attached from a socket address.
        ///
        /// This function demonstrates creating native backends at initalization.
        /// The backend is for the VCL lifetime and can be reused..
        pub fn new(
            ctx: &mut Ctx,
            #[vcl_name] vcl_name: &str,
            addr: &str,
            probe: Option<Probe>,
        ) -> Result<Self, &'static str> {
            let Ok(sock_addr) = addr.parse() else {
                return Err("failed to parse addr");
            };

            // Create backend name from address
            let name = format!("{vcl_name}_{sock_addr}");
            let Ok(name_cstr) = std::ffi::CString::new(name) else {
                return Err("failed to create native name");
            };

            let native_backend_probe = match probe {
                None => ffi::VCL_PROBE(null()),
                Some(probe) => probe.into_vcl(&mut ctx.ws).expect("no workspace left"),
            };

            let Ok(native_backend) = NativeBackendBuilder::new_ip(&name_cstr, sock_addr)
                .probe(&native_backend_probe)
                .build(ctx)
            else {
                return Err("failed to create native backend");
            };

            Ok(Self {
                backend: native_backend,
            })
        }

        /// Retrieve the configured native backend.
        pub fn backend(&self) -> BackendRef {
            self.backend.as_ref().clone()
        }
    }

    /// Create a dynamic backend from a socket address.
    ///
    /// This function demonstrates creating native backends at runtime.
    /// The backend is stored per-task and reused within the same request.
    ///
    /// Example:
    /// ```vcl
    /// set req.backend_hint = native_backend.create("${server_addr}:${server_port}");
    /// ```
    pub fn create(
        ctx: &mut Ctx,
        /// Socket address string (e.g., "127.0.0.1:8080")
        addr: Option<&str>,
        /// Storage for the created native backend (per-task)
        #[shared_per_task]
        backend_storage: &mut Option<Box<NativeBackend>>,
    ) -> Option<BackendRef> {
        // Parse the socket address string
        let addr_str = addr?;
        let sock_addr: SocketAddr = addr_str.parse().ok()?;

        // Create backend name from address
        let name = format!("native_{sock_addr}");
        let name_cstr = std::ffi::CString::new(name).ok()?;

        // Build the backend
        let native_backend = NativeBackendBuilder::new_ip(&name_cstr, sock_addr)
            .build(ctx)
            .ok()?;

        let backend_ref = native_backend.as_ref().clone();
        *backend_storage = Some(Box::new(native_backend));
        Some(backend_ref)
    }
}
