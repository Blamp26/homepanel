use tower_http::{set_status::SetStatus, services::{ServeDir, ServeFile}};

pub fn service() -> ServeDir<SetStatus<ServeFile>> {
    ServeDir::new("frontend/dist").not_found_service(ServeFile::new("frontend/dist/index.html"))
}
