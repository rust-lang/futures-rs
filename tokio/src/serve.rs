use std::time::Duration;

use mio_server;
use Service;

#[allow(dead_code)]
pub struct ServerConfig {
    name: Option<String>,
    keep_alive: Option<bool>,
    max_requests: Option<u32>,
    request_timeout: Option<Duration>,
    read_timeout: Option<Duration>,
    write_completion_timeout: Option<Duration>,
    max_host_idle_time: Option<Duration>,
    max_host_life_time: Option<Duration>,
}

impl Default for ServerConfig {
    fn default() -> ServerConfig {
        ServerConfig {
            name: None,
            keep_alive: None,
            max_requests: None,
            request_timeout: None,
            read_timeout: None,
            write_completion_timeout: None,
            max_host_life_time: None,
            max_host_idle_time: None,
        }
    }
}

pub trait Serve {
    type Req: Send + 'static;
    type Resp: Send + 'static;
    type Error: Send + 'static;

    fn get_config_mut(&mut self) -> &mut ServerConfig;
    fn get_mio_mut(&mut self) -> &mut mio_server::Server;

    fn serve_unconfigured<S>(&mut self, S) -> Result<(), Self::Error>
        where S: Service<Req = Self::Req, Resp = Self::Resp, Error = Self::Error>;

    fn name(&mut self, name: String) -> &mut Self {
        self.get_config_mut().name = Some(name);
        self
    }

    fn keep_alive(&mut self, keep_alive: bool) -> &mut Self {
        self.get_config_mut().keep_alive = Some(keep_alive);
        self
    }

    fn workers(&mut self, workers: u32) -> &mut Self {
        self.get_mio_mut().workers = workers;
        self
    }

    // TODO: maybe assoc type for success?
    fn serve<S>(&mut self, service: S) -> Result<(), Self::Error>
        where S: Service<Req = Self::Req, Resp = Self::Resp, Error = Self::Error>
    {
        self.serve_unconfigured(service)
    }
}
