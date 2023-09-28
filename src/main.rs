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
    Answer, CorrectionRequest, EmptyResponse, EvaluateRequest, GenerateKnowledgeRequest, Thematics,
};
use tracing_subscriber::prelude::*;

pub struct TirServer {
    gpt: tirengine::GPT,
}

#[tonic::async_trait]
impl TirService for TirServer {
    #[tracing::instrument(skip(self))]
    async fn generate_knowledge(
        &self,
        request: Request<GenerateKnowledgeRequest>,
    ) -> Result<Response<Thematics>, Status> {
        // FIXME: really awkward API design in tir-engine
        // we shouldn't mutate the requested thing, just return the generated data..
        // This is just horrible to map back and forth between gRPC and tir-engine structs
        let GenerateKnowledgeRequest { thematic } = request.into_inner();
        let thematic = thematic.ok_or_else(|| Status::unavailable("missing thematic"))?;
        let topics = thematic
            .topics
            .into_iter()
            .map(|topic| tirengine::Topic {
                explanation: Some(topic.explanation),
                title: topic.title,
            })
            .collect();
        let mut thematic = tirengine::Thematic {
            title: thematic.title,
            topics,
        };
        self.gpt
            .generate_knowledge(&mut thematic)
            .await
            .map_err(|e| {
                let msg = format!("tir engine failed to generate knowledge, details:\n{:?}", e);
                Status::unavailable(msg)
            })?;

        let topics: Result<Vec<_>, _> =
            thematic.topics.into_iter().map(TryFrom::try_from).collect();

        // Probably it doesn't make sense to return a vector of thematics, but nvm..
        let thematics = tir_grpc::Thematics {
            thematics: vec![tir_grpc::Thematic {
                topics: topics?,
                title: thematic.title,
            }],
        };

        Ok(Response::new(thematics))
    }

    #[tracing::instrument(skip(self))]
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

        let answer = self.gpt.evaluate_answer(answer, topic).await.map_err(|e| {
            let msg = format!("tir engine failed to evaluate answer, details:\n{:?}", e);
            Status::unavailable(msg)
        })?;

        Ok(Response::new(tir_grpc::Answer::from(answer)))
    }

    #[tracing::instrument(skip(self))]
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

        self.gpt
            .correct_explanation(correction, &mut topic)
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
                .unwrap_or_else(|_| "tirengine=debug,tir_engine_grpc=debug,tonic=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    let port = std::env::var("PORT").unwrap_or_else(|_| "50051".into());
    let addr = format!("0.0.0.0:{port}").parse()?;
    let secret = std::env::var("OPENAI_SK").expect("OPENAI_SK should be set");

    assert_ne!(secret.as_str(), "", "OPENAI_SK should not be empty");

    let tir_server = TirServer {
        gpt: tirengine::GPT::new(secret),
    };

    tracing::debug!(
        addr = ?addr,
        "Starting TIR gRPC service..",
    );

    Server::builder()
        .add_service(TirServiceServer::new(tir_server))
        .serve_with_shutdown(addr, shutdown_signal())
        .await?;

    Ok(())
}

pub async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::warn!("signal received, shutting down..");
}
