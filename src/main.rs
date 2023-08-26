pub mod tir_grpc {
    #![allow(non_snake_case)]
    tonic::include_proto!("tir");

    impl From<tirengine::Answer> for self::Answer {
        fn from(value: tirengine::Answer) -> Self {
            Self {
                score: u32::from(value.score),
                explanation: value.explanation,
            }
        }
    }

    impl TryFrom<tirengine::Topic> for self::Topic {
        type Error = tonic::Status;

        fn try_from(value: tirengine::Topic) -> Result<Self, Self::Error> {
            Ok(Self {
                title: value.title,
                explanation: value
                    .explanation
                    .ok_or_else(|| tonic::Status::invalid_argument("explanation missing"))?,
            })
        }
    }
}

use tonic::{transport::Server, Request, Response, Status};

use tir_grpc::tir_service_server::{TirService, TirServiceServer};
use tir_grpc::{
    Answer, CorrectionRequest, EmptyRequest, EmptyResponse, EvaluateRequest, Thematics,
};
use tracing_subscriber::prelude::*;

#[derive(Debug, Default)]
pub struct TirServer {}

#[tonic::async_trait]
impl TirService for TirServer {
    #[tracing::instrument]
    async fn generate_knowledge(
        &self,
        _request: Request<EmptyRequest>,
    ) -> Result<Response<Thematics>, Status> {
        let thematics = tirengine::generate_knowledge().await.map_err(|e| {
            let msg = format!("tir engine failed to generate knowledge, details:\n{:?}", e);
            Status::unavailable(msg)
        })?;

        let thematics = thematics
            .into_iter()
            .map(|thematic| {
                let topics: Vec<_> = thematic
                    .topics
                    .into_iter()
                    .map(TryFrom::try_from)
                    .collect::<Result<Vec<_>, Status>>()
                    // I believe it's ok to unwrap, we error before this anyway if there's anything wrong
                    .unwrap();
                tir_grpc::Thematic {
                    title: thematic.title,
                    topics,
                }
            })
            .collect();

        Ok(Response::new(tir_grpc::Thematics { thematics }))
    }

    #[tracing::instrument]
    async fn evaluate_answer(
        &self,
        evaluate_request: Request<EvaluateRequest>,
    ) -> Result<Response<Answer>, Status> {
        let EvaluateRequest { answer, topic } = evaluate_request.into_inner();
        let topic = topic.ok_or_else(|| Status::invalid_argument("missing topic"))?;
        let topic = tirengine::Topic {
            title: topic.title,
            explanation: Some(topic.explanation),
        };

        let answer = tirengine::evaluate_answer(answer, topic)
            .await
            .map_err(|e| {
                let msg = format!("tir engine failed to evaluate answer, details:\n{:?}", e);
                Status::unavailable(msg)
            })?;

        Ok(Response::new(tir_grpc::Answer::from(answer)))
    }

    #[tracing::instrument]
    async fn correct_explanation(
        &self,
        request: Request<CorrectionRequest>,
    ) -> Result<Response<EmptyResponse>, Status> {
        let CorrectionRequest { correction, topic } = request.into_inner();
        let topic = topic.ok_or_else(|| Status::invalid_argument("missing topic"))?;
        let mut topic = tirengine::Topic {
            title: topic.title,
            explanation: Some(topic.explanation),
        };

        tirengine::correct_explanation(correction, &mut topic)
            .await
            .map_err(|e| {
                let msg = format!(
                    "tir engine failed to generate correct explanation, details:\n{:?}",
                    e
                );
                Status::unavailable(msg)
            })?;

        Ok(Response::new(EmptyResponse {}))
    }
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tirengine=debug,tir-engine-grpc=debug,tonic=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    let addr = "[::1]:50051".parse()?;
    let greeter = TirServer::default();

    Server::builder()
        .add_service(TirServiceServer::new(greeter))
        .serve(addr)
        .await?;

    Ok(())
}
