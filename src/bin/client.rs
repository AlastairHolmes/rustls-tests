use io::Read;
use io::Write;
use std::io;
use std::sync::Arc;

fn main() {
    let mut root_store = rustls::RootCertStore::empty();
    root_store.add_server_trust_anchors(
        webpki_roots::TLS_SERVER_ROOTS
            .0
            .iter()
            .map(|ta| {
                rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
                    ta.subject,
                    ta.spki,
                    ta.name_constraints,
                )
            }),
    );
    let config = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();

	let server_name = "google.com".try_into().unwrap();
	let mut conn = rustls::ClientConnection::new(Arc::new(config), server_name).unwrap();
	let mut sock = std::net::TcpStream::connect("google.com:443").unwrap();
	let mut tls = rustls::Stream::new(&mut conn, &mut sock);
	tls.write_all(
		concat!(
			"GET / HTTP/1.1\r\n",
			"Host: google.com\r\n",
			"Connection: close\r\n",
			"Accept-Encoding: identity\r\n",
			"\r\n"
		)
		.as_bytes(),
	)
	.unwrap();
	let ciphersuite = tls
		.conn
		.negotiated_cipher_suite()
		.unwrap();
	writeln!(
		&mut std::io::stderr(),
		"Current ciphersuite: {:?}",
		ciphersuite.suite()
	)
	.unwrap();
	let mut plaintext = Vec::new();
	tls.read_to_end(&mut plaintext).unwrap();
	io::stdout().write_all(&plaintext).unwrap();
}
