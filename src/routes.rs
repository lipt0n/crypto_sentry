use rocket::Rocket;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[rocket::main]
pub async fn rocket() -> Result<(), rocket::Error> {
    rocket::build().mount("/", routes![index]).launch().await
}
