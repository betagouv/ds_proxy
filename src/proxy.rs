use super::key::*;
use super::encrypt::*;
use super::decoder::*;
use actix_web::client::Client;
use actix_web::{middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer};
use futures::Future;
use actix_web::guard;

fn forward(
    req: HttpRequest,
    payload: web::Payload,
    client: web::Data<Client>,
    upstream_base_url: web::Data<String>,
) -> impl Future<Item = HttpResponse, Error = Error> {

    let key = build_key();
    let encoder = Encoder::new(key, 512, Box::new(payload));

    let put_url = format!("{}{}", upstream_base_url.get_ref(), req.uri());

    client.put(put_url)
        .header("User-Agent", "Actix-web")
        .send_stream(encoder)
        .map_err(|e| {
            println!("==== erreur1 ====");
            println!("{:?}", e);
            Error::from(e)
        })
    .map(|res| {
        let mut client_resp = HttpResponse::build(res.status());
        for (header_name, header_value) in
            res.headers().iter().filter(|(h, _)| *h != "connection")
            {
                client_resp.header(header_name.clone(), header_value.clone());
            }
        client_resp.streaming(res)
    })
}

fn fetch(
    req: HttpRequest,
    client: web::Data<Client>,
    upstream_base_url: web::Data<String>,
) -> impl Future<Item = HttpResponse, Error = Error> {

    let get_url = format!("{}{}", upstream_base_url.get_ref(), req.uri());

    client.get(get_url)
        .header("User-Agent", "Actix-web")
        .send()
        .map_err(|e| {
            println!("==== erreur1 ====");
            println!("{:?}", e);
            Error::from(e)
        })
    .map(|res| {
        let mut client_resp = HttpResponse::build(res.status());
        for (header_name, header_value) in
            res.headers().iter().filter(|(h, _)| *h != "connection")
            {
                client_resp.header(header_name.clone(), header_value.clone());
            }

        let key = build_key();
        let decoder = Decoder::new(key, 512, Box::new(res));

        client_resp.streaming(decoder)
    })
}


pub fn main(listen_addr: &str, listen_port: u16, upstream_base_url: String) -> std::io::Result<()> {
    HttpServer::new(move || {
        App::new()
            .data(actix_web::client::Client::new())
            .data(upstream_base_url.clone())
            .wrap(middleware::Logger::default())
            .service(web::resource(".*").guard(guard::Get()).to_async(fetch))
            .default_service(web::route().guard(guard::Put()).to_async(forward))
    })
    .bind((listen_addr.as_ref(), listen_port))?
        .system_exit()
        .run()
}
