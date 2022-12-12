use futures::SinkExt;
use futures::StreamExt;
use futures::TryFutureExt;
use io::Read;
use io::Write;
use rustls::ServerConfig;
use tokio::io::AsyncReadExt;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use tokio;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
	todo!()
}

type PeerId = u64;

enum PeerCertificateChange {
	Add(PeerId, std::net::SocketAddr, rustls::Certificate),
	Remove(PeerId)
}

enum PeerDesignation {
	/// A connection to a peer where we are the client and they are the server
	ServerPeer,
	/// A connection to a peer where we are the server and they are the client
	ClientPeer,
}

struct PeerConnection {
	task_handle: tokio::task::JoinHandle<()>,
	outgoing_frame_sender: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
	peer_designation: PeerDesignation
}
impl PeerConnection {
	fn new_client_peer_connection(peer_id: PeerId, incoming_frame_sender: tokio::sync::mpsc::UnboundedSender<(PeerId, Vec<u8>)>, new_connection: quinn::NewConnection) -> Self {
		let (outgoing_frame_sender, mut outgoing_frame_receiver) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
	
		PeerConnection {
			task_handle: tokio::spawn(async move {
				let _connection = new_connection.connection;
				let mut new_bi_streams = new_connection.bi_streams;
				let mut option_frame_bi_stream: Option<(tokio_util::codec::FramedWrite<quinn::SendStream, tokio_util::codec::LengthDelimitedCodec>, tokio_util::codec::FramedRead<quinn::RecvStream, tokio_util::codec::LengthDelimitedCodec>)> = None;

				loop {
					if let Some((frame_send_stream, frame_receive_stream)) = option_frame_bi_stream.as_mut() {
						option_frame_bi_stream = loop {
							let test = async {
								loop {
									tokio::select! {
										Some(outgoing_frame) = outgoing_frame_receiver.recv() => {
											
											frame_send_stream.send(outgoing_frame.into()).await;
											frame_send_stream.flush().await;
											// TODO
										}
									}
								}
							};

							tokio::select! {
								_ = test => {},
								option_result_bi_stream = new_bi_streams.next() => {
									if let Some(Ok(bi_stream)) = option_result_bi_stream {
										// TODO: Send Connected on incoming channel
										break Some(Self::bi_stream_into_bi_frame_stream(bi_stream));
									} else {
										// FAILURE
										// break;
									}
								},
								Some(result_incoming_frame) = frame_receive_stream.next() => {
									match result_incoming_frame {
										Ok(incoming_frame) => {
											incoming_frame_sender.send((peer_id, incoming_frame.into())); // TODO: Use BytesMut ?
										},
										Err(error) => {
											// FAILURE Log
											break None;
										}
									}
								}
							}
						}
					} else {
						tokio::select! {
							option_result_bi_stream = new_bi_streams.next() => {
								if let Some(Ok(bi_stream)) = option_result_bi_stream {
									// TODO: Send Connected on incoming channel
									option_frame_bi_stream = Some(Self::bi_stream_into_bi_frame_stream(bi_stream));
								} else {
									// FAILURE
									break;
								}
							},
							Some(frame) = outgoing_frame_receiver.recv() => {
								// Just drop frames as we aren't connected
							}
						}
					}
				}
			}),
			outgoing_frame_sender,
			peer_designation: PeerDesignation::ClientPeer,
		}
	}

	fn new_server_peer_connection(
		endpoint: quinn::Endpoint,
		our_certificate_chain: Vec<rustls::Certificate>,
		our_private_key: rustls::PrivateKey,
		peer_address: std::net::SocketAddr,
		peer_certificate: rustls::Certificate,
	) -> Self {
		let (outgoing_frame_sender, outgoing_frame_receiver) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

		let client_crypto = rustls::ClientConfig::builder()
			.with_safe_defaults()
			.with_root_certificates({
				let mut root_cert_store = rustls::RootCertStore::empty();
				// webpki::TrustAnchor::try_from_cert_der(certificate.as_ref());
				let _result = root_cert_store.add(&peer_certificate); // FAILURE LOG
				root_cert_store
			})
			.with_single_cert(our_certificate_chain.clone(), our_private_key.clone()).unwrap();

		PeerConnection {
			task_handle: tokio::spawn(async move {
				let client_config = Arc::new(client_crypto);
				loop {
					let pending_connection = endpoint.connect_with(quinn::ClientConfig::new(client_config.clone()), peer_address, "chainflip.com").unwrap(); // TODO: Check this name is ok

					let new_connection = pending_connection.await.unwrap();
					let connection = new_connection.connection; 

					let (send_frame_stream, mut receive_frame_stream) = Self::bi_stream_into_bi_frame_stream(connection.open_bi().await.unwrap());

					// TODO Need to drop messages from outgoing_frame_sender while waiting for bi stream to be opened

					// TODO: Add buffer into LengthDelimitedCodec
					// TODO: Possible add a tokio::io::BufReader to the receive_stream. Seems that quinn has internal buffering, but I need to confirm this.
					
					loop {
						tokio::select! {
							_ = receive_frame_stream.next() => {

							}
						}
					}

					// sleep instead of immediately reconnecting
				}
			}),
			outgoing_frame_sender,
			peer_designation: PeerDesignation::ServerPeer
		}
	}

	fn bi_stream_into_bi_frame_stream((send_bytes_stream, receive_bytes_stream): (quinn::SendStream, quinn::RecvStream)) -> (
		tokio_util::codec::FramedWrite<quinn::SendStream, tokio_util::codec::LengthDelimitedCodec>,
		tokio_util::codec::FramedRead<quinn::RecvStream, tokio_util::codec::LengthDelimitedCodec>
	) {
		(
			tokio_util::codec::LengthDelimitedCodec::builder()
				.max_frame_length(todo!())
				.new_write(send_bytes_stream),
			tokio_util::codec::LengthDelimitedCodec::builder()
				.max_frame_length(todo!())
				.new_read(receive_bytes_stream)
		)
	}
}

struct Peer {
	peer_cert_change_sender: tokio::sync::mpsc::UnboundedSender<PeerCertificateChange>,
	outgoing_frame_sender: tokio::sync::mpsc::UnboundedSender<(PeerId, Vec<u8>)>,
	incoming_frame_receiver: tokio::sync::mpsc::UnboundedReceiver<(PeerId, Vec<u8>)>, // Unbounded Questionable?
}

impl Peer {
	async fn start<FnServerClientPeerDesignator: Fn(&PeerId, &PeerId) -> PeerDesignation + 'static + Send + Sync>(
		our_peer_id: PeerId,
		our_endpoint_address: std::net::SocketAddr,
		our_certificate_chain: Vec<rustls::Certificate>,
		our_private_key: rustls::PrivateKey,
		server_client_designator: FnServerClientPeerDesignator,
	) -> Result<Self, anyhow::Error> {
		// TODO: Only use one channel?
		let (peer_cert_change_sender, mut peer_cert_change_receiver) = tokio::sync::mpsc::unbounded_channel::<PeerCertificateChange>();
		let (outgoing_frame_sender, outgoing_frame_receiver) = tokio::sync::mpsc::unbounded_channel::<(PeerId, Vec<u8>)>();
		let (incoming_frame_sender, incoming_frame_receiver) = tokio::sync::mpsc::unbounded_channel::<(PeerId, Vec<u8>)>();

		let mut client_peer_to_cert_mapping = BTreeMap::<PeerId, rustls::Certificate>::new();
		let mut cert_to_client_peer_mapping = BTreeMap::<rustls::Certificate, PeerId>::new();

		// Connections to "client peers" that have not finished their handshake
		let mut pending_client_peer_connections = futures::stream::FuturesUnordered::new();
		let mut peer_connections = BTreeMap::<PeerId, PeerConnection>::new();

		let new_server_config = {
			let our_certificate_chain = our_certificate_chain.clone();
			let our_private_key = our_private_key.clone();
			move |peer_cert_mapping: &BTreeMap<PeerId, rustls::Certificate>| -> Result<quinn::ServerConfig, rustls::Error> {
				Ok(quinn::ServerConfig::with_crypto(Arc::new(rustls::ServerConfig::builder()
					.with_safe_defaults()
					.with_client_cert_verifier(rustls::server::AllowAnyAuthenticatedClient::new({
						let mut root_cert_store = rustls::RootCertStore::empty();
						for (_, certificate) in peer_cert_mapping {
							// webpki::TrustAnchor::try_from_cert_der(certificate.as_ref());
							let _result = root_cert_store.add(certificate); // FAILURE LOG
						}
						root_cert_store
					}))
					.with_single_cert(our_certificate_chain.clone(), our_private_key.clone())?)))
			}
		};

		// TODO: Portable applications should bind an address that matches the family they wish to communicate within.
		let (endpoint, mut incoming) = quinn::Endpoint::server(new_server_config(&client_peer_to_cert_mapping)?, our_endpoint_address)?;

		tokio::spawn(async move {
			loop {
				tokio::select! {
					option_connecting = incoming.next() => {
						if let Some(connecting) = option_connecting {
							// TODO: May need to limit number of pending connections
							pending_client_peer_connections.push(connecting);
						} else {
							// FAILURE return error
							break;
						}
					},
					Some(result_connection) = pending_client_peer_connections.next() => {
						match result_connection {
							Ok(new_connection) => {
								if let Some(peer_root_cert) = new_connection.connection.peer_identity().and_then(|certificates| {
									certificates.downcast::<Vec<rustls::Certificate>>().ok().and_then(|certificates| certificates.last().cloned())
								}) {
									if let Some(peer_id) = cert_to_client_peer_mapping.get(&peer_root_cert) {
										peer_connections.insert(*peer_id, PeerConnection::new_client_peer_connection(*peer_id, incoming_frame_sender.clone(), new_connection));
									} else {
										// FAILURE log?
									}
								} else {
									// FAILURE log?
								}
							},
							Err(error) => {
								// FAILURE log
							}
						}
					},
					Some(peer_cert_change) = peer_cert_change_receiver.recv() => {
						match peer_cert_change {
							PeerCertificateChange::Add(peer_id, peer_address, peer_certificate) => {
								match server_client_designator(&our_peer_id, &peer_id) {
									PeerDesignation::ServerPeer => {
										peer_connections.insert(peer_id, PeerConnection::new_server_peer_connection(
											endpoint.clone(),
											our_certificate_chain.clone(),
											our_private_key.clone(),
											peer_address,
											peer_certificate,
										));		
									},
									PeerDesignation::ClientPeer => {
										if client_peer_to_cert_mapping.get(&peer_id) != Some(&peer_certificate) {
											peer_connections.remove(&peer_id); // Force peer to reconnect with new certificate
											if let Some(old_certificate) = client_peer_to_cert_mapping.insert(peer_id, peer_certificate.clone()) {
												cert_to_client_peer_mapping.remove(&old_certificate);
											}
											assert!(cert_to_client_peer_mapping.insert(peer_certificate, peer_id).is_none()); // Certificates must be unique for each peer
											endpoint.set_server_config(Some(new_server_config(&client_peer_to_cert_mapping).unwrap()));
										}
									}
								}
							},
							PeerCertificateChange::Remove(peer_id) => {
								if let Some(PeerConnection { peer_designation: PeerDesignation::ClientPeer, .. }) = peer_connections.remove(&peer_id) { // TODO: Possibly allow connection to continue for a small period with cleaner shutdown?
									if let Some(certificate) = client_peer_to_cert_mapping.remove(&peer_id) {
										cert_to_client_peer_mapping.remove(&certificate);
									}
									endpoint.set_server_config(Some(new_server_config(&client_peer_to_cert_mapping).unwrap()));
								}
							}
						}	
					}
					// Forward messages
				}
			}
		});

		Ok(Self {
			peer_cert_change_sender,
			outgoing_frame_sender,
			incoming_frame_receiver,
		})
	}
}