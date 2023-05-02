use std::net::{Ipv6Addr, SocketAddr, SocketAddrV6};

use hello_world::{
    greeter_server::{Greeter, GreeterServer},
    HelloReply, HelloRequest,
};
use hyper_rustls::ConfigBuilderExt;
use serde::{Deserialize, Serialize};
use tonic::{transport::Server, Request, Response, Status};

pub mod hello_world {
    tonic::include_proto!("helloworld");
}

const STRIPE_SECRET: &str = "sk_test_51J6o2xBygty0Fo15FUJsbXYx7dIiiw4Ou1JEdKtzo1mOxEKnTgzWnPmhcsNJWK2DEIGHFCWWafRnUezHLTT3SxlT00Y7rfnAOj";

#[derive(Debug)]
pub struct MyGreeter {
    stripe_client: hyper::client::Client<
        hyper_rustls::HttpsConnector<hyper::client::HttpConnector>,
        hyper::Body,
    >,
}

#[tonic::async_trait]
impl Greeter for MyGreeter {
    async fn say_hello(
        &self,
        request: Request<HelloRequest>, // Accept request of type HelloRequest
    ) -> Result<Response<HelloReply>, Status> {
        // Return an instance of type HelloReply
        println!("request: {:?}", request);
        let inner = request.into_inner();

        // https://stripe.com/docs/payments/save-and-reuse

        // Create `Customer`
        // https://stripe.com/docs/payments/save-and-reuse?platform=web#create-customer
        // https://stripe.com/docs/api/customers/create
        let customer_id = {
            let mut json = serde_json::to_value(&inner.customer).unwrap();
            let req = hyper::Request::post("https://api.stripe.com/v1/customers")
                .header("user", format!("{STRIPE_SECRET}:"))
                .body(hyper::Body::from(json.to_string()))
                .unwrap();
            let res = self.stripe_client.request(req).await.unwrap();
            let body = res.collect().await.unwrap().aggregate();
            let customer: serde_json::Value = serde_json::from_reader(body.reader()).unwrap();
            let customer_id = customer
                .as_object()
                .unwrap()
                .get("id")
                .unwrap()
                .as_str()
                .unwrap();
            customer_id
        };
        println!("customer_id: {customer_id}");

        // Create card
        // https://stripe.com/docs/api/cards/create
        let payment_id = {
            let mut json = serde_json::to_value(&inner.card).unwrap();
            json.as_object_mut()
                .unwrap()
                .insert(
                    String::from("object"),
                    serde_json::Value::String(String::from("card")),
                )
                .unwrap();
            let source = serde_json::json!({ "source": json });
            let req = hyper::Request::post(format!(
                "https://api.stripe.com/v1/customers/{customer_id}/sources"
            ))
            .header("user", format!("{STRIPE_SECRET}:"))
            .body(hyper::Body::from(source.to_string()))
            .unwrap();
            let res = self.stripe_client.request(req).await.unwrap();
            let body = res.collect().await.unwrap().aggregate();
            let payment: serde_json::Value = serde_json::from_reader(body.reader()).unwrap();
            let payment_id = payment
                .as_object()
                .unwrap()
                .get("id")
                .unwrap()
                .as_str()
                .unwrap();
            payment_id
        };
        println!("payment_id: {payment_id}");

        // Create `SetupIntent`
        // https://stripe.com/docs/payments/save-and-reuse?platform=web#web-create-intent
        // https://stripe.com/docs/api/setup_intents/create
        {
            let body = serde_json::json! ({
                "confirm": true,
                "customer": customer_id,
                "payment_method": payment_id,
                "payment_method_types": ["card"]
            });
            let req = hyper::Request::builder()
                .header("user", format!("{STRIPE_SECRET}:"))
                .body(hyper::Body::from(body.to_string()))
                .unwrap();
            let res = self.stripe_client.request(req).await.unwrap();
            let body = res.collect().await.unwrap().aggregate();
            let setup_intent: serde_json::Value = serde_json::from_reader(body.reader()).unwrap();
            println!("setup_intent: {setup_intent}");
        }

        // Create test payment
        {
            
        }
        
        let reply = hello_world::HelloReply {
            hello: format!("Hello {}", request.get_ref().name),
        };

        Ok(Response::new(reply)) // Send back our formatted greeting
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct User {
    hash: String,
    // TODO Use `SocketAddrV6` or a stricter type
    repo: Option<String>,
    instance: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (address, port) = (Ipv6Addr::LOCALHOST, 8080);

    // See https://github.com/rustls/hyper-rustls/blob/main/examples/client.rs
    let tls = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_native_roots()
        .with_no_client_auth();
    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config(tls)
        .https_or_http()
        .enable_http1()
        .build();
    let client: hyper::client::Client<_, hyper::Body> =
        hyper::client::Client::builder().build(https);

    let greeter = MyGreeter {
        stripe_client: client,
    };
    let addr = SocketAddr::V6(SocketAddrV6::new(address, port, 0, 0));

    Server::builder()
        .add_service(GreeterServer::new(greeter))
        .serve(addr)
        .await?;

    Ok(())
}
