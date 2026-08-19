fn main() {
    let opts = gizza_ai_ssh_public_key_parser_core::Options {
        include_sha1: false,
        uppercase_md5: false,
        expected_fingerprint: String::new(),
        now: 0,
    };
    println!("{}", gizza_ai_ssh_public_key_parser_core::run("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIPc21YeL9wdmn0Bvy1dVCZH/rO/hcbVFBt5YQ/Y8+oOy alice@example.com", &opts).unwrap());
}
