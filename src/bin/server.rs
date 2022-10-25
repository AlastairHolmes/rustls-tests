use futures::StreamExt;
use io::Read;
use io::Write;
use rustls::ServerConfig;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use tokio;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {


    /*let tcp_listener = tokio::net::TcpListener::bind("0.0.0.0:443").await.unwrap();

	let (stream, addr) = tcp_listener.accept().await.unwrap();

	let config = rustls::ServerConfig::builder()
		.with_safe_defaults()
		.with_no_client_auth()
		.with_single_cert(todo!(), todo!())
		.unwrap();
	let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
	let t = acceptor.accept(stream).await;



	rustls::server::AllowAnyAuthenticatedClient::new(rustls::RootCertStore::empty());
*/
	todo!()
}


type AccountId = u64;

enum PeerChange {
	Add(AccountId, rustls::Certificate),
	Remove(AccountId)
}

// (AccountId, Message)

struct TcpServer {
	peer_change_sender: tokio::sync::mpsc::Sender<PeerChange>,
	outgoing_message_sender: tokio::sync::mpsc::Sender<(AccountId, Vec<u8>)>,
	incoming_message_receiver: tokio::sync::mpsc::Receiver<(AccountId, Vec<u8>)>,
}

struct AccountHandle {


}

impl TcpServer {
	async fn start(
		identity: (rustls::Certificate, rustls::PrivateKey),
		addr: std::net::SocketAddr,
	) -> Result<(), anyhow::Error> {
		// Cert must be unique per peer

		let (peer_change_sender, mut peer_change_receiver) = tokio::sync::mpsc::unbounded_channel::<PeerChange>();

		let mut account_cert_mapping = BTreeMap::<AccountId, rustls::Certificate>::new();
		let mut cert_account_mapping = BTreeMap::<rustls::Certificate, AccountId>::new();

		let new_server_config = move |account_cert_mapping: &BTreeMap<AccountId, rustls::Certificate>| -> Result<quinn::ServerConfig, rustls::Error> {
			Ok(quinn::ServerConfig::with_crypto(Arc::new(rustls::ServerConfig::builder()
				.with_safe_defaults()
				.with_client_cert_verifier(rustls::server::AllowAnyAuthenticatedClient::new({
					let mut root_cert_store = rustls::RootCertStore::empty();
					for (_, certificate) in account_cert_mapping {
						root_cert_store.add(certificate); // TODO result
					}
					root_cert_store
				}))
				.with_single_cert(vec![identity.0.clone()], identity.1.clone())?)))
		};

		let (endpoint, mut incoming) = quinn::Endpoint::server(new_server_config(&account_cert_mapping)?, addr)?;

		let mut pending_connections = futures::stream::FuturesUnordered::new();

		let peers = BTreeMap::<AccountId, ()>::new();
			// outgoing message sender
			// new connection sender
			// 

		tokio::spawn(async move {
			loop {
				tokio::select! {
					option_connecting = incoming.next() => {
						if let Some(connecting) = option_connecting {
							let connecting: quinn::Connecting = connecting;
							// TODO: May need to limit number of pending connections
							pending_connections.push(connecting);
						} else {
							break;
						}
					},
					Some(result_connection) = pending_connections.next() => {
						match result_connection {
							Ok(connection) => {
								if let Some(peer_identity) = connection.connection.peer_identity().and_then(|peer_identity| {
									peer_identity.downcast::<rustls::Certificate>().ok()
								}) {
									if let Some(account_id) = cert_account_mapping.get(&peer_identity) {
										let quinn::NewConnection { connection, mut bi_streams, .. } = { connection };

										// set of threads one for each account id
											// created on send attempt or connection
												// we need a map of account id to thread handles
											// tell it about new connections 

										tokio::spawn(async move {
											// Pending streams in FuturesUnordered
											// Identified streams in FuturesUnordered
											// 



											loop {
												tokio::select! {
													Some(bi_stream) = bi_streams.next() => {

													}
													// New connection
													// Receive Outgoing
													// Wait on all streams
														// Into bounded channel 
												}
											}


											// Find New Connection
											// Messages to send (Sender is in the map/thread handle)
											// Time out messages

											// or we could have the p2p core handle contain map of channels (one for each peer)
												// advantage direct channels
										});

										// need connection outside so it can be dropped
										// clone

										// sending
											// buffered on our side for period


										// create channels for each account id ???	

										// set concurrent stremas max ???
										// threads for each bi_streams
											// receive a bi-stream ID it

										// store connections
											// in a map from AccountId to Connection
									} else {
										// FAILURE
									}
								} else {
									// FAILURE
								}
							},
							Err(error) => {
								// FAILURE
							}
						}
					},
					Some(peer_change) = peer_change_receiver.recv() => {
						// drop connections for removed certificate

						match peer_change {
							PeerChange::Add(account_id, certificate) => {
								if account_cert_mapping.get(&account_id) != Some(&certificate) {
									if let Some(old_certificate) = account_cert_mapping.insert(account_id, certificate.clone()) {
										cert_account_mapping.remove(&old_certificate);
									}
									assert!(cert_account_mapping.insert(certificate, account_id).is_none()); // Certificates must be unique for each peer
								}
							},
							PeerChange::Remove(account_id) => {
								if let Some(certificate) = account_cert_mapping.remove(&account_id) {
									cert_account_mapping.remove(&certificate);
								}
							}
						}
						endpoint.set_server_config(Some(new_server_config(&account_cert_mapping).unwrap()));
					}
				}

			}
		});

		todo!()
	}
}