use actix_web::dev::{Service, ServiceRequest, ServiceResponse};
use std::rc::Rc;

/// Request logger middleware.
pub struct Logger;

impl<S, B> actix_web::dev::Transform<S, ServiceRequest> for Logger
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type InitError = ();
    type Transform = LoggerMiddleware<S>;
    type Future = core::future::Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        core::future::ready(Ok(LoggerMiddleware {
            service: Rc::new(service),
        }))
    }
}

pub struct LoggerMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for LoggerMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Future =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let svc = self.service.clone();
        let method = req.method().clone();
        let path = req.path().to_string();
        Box::pin(async move {
            log::info!("{} {}", method, path);
            let start = std::time::Instant::now();
            let res = svc.call(req).await?;
            log::info!(
                "  -> {} {} in {:.2}ms",
                res.status(),
                path,
                start.elapsed().as_millis()
            );
            Ok(res)
        })
    }
}
