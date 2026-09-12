//! Discovers Protobuf packages and generates their Rust modules, service traits,
//! direct and remote clients, routers, and descriptors. `RpcChannel` carries
//! encoded values only at remote boundaries; direct clients pass typed values.

use prost_build::{Method, Service, ServiceGenerator};
use std::fmt::Write;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct ArutServiceGenerator;

pub fn compile_dir(root: impl AsRef<Path>) -> io::Result<()> {
    let root = root.as_ref();
    println!("cargo:rerun-if-changed={}", root.display());
    let mut protos = Vec::new();
    collect_protos(root, &mut protos)?;
    protos.sort();
    if protos.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no .proto files found under {}", root.display()),
        ));
    }

    let descriptors = protox::compile(&protos, [root]).map_err(io::Error::other)?;
    let mut config = prost_build::Config::new();
    config.include_file("arut.protocols.rs");
    config.service_generator(Box::new(ArutServiceGenerator));
    config.compile_fds(descriptors)
}

fn collect_protos(directory: &Path, protos: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_protos(&path, protos)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "proto")
        {
            protos.push(path);
        }
    }
    Ok(())
}

impl ServiceGenerator for ArutServiceGenerator {
    fn generate(&mut self, service: Service, output: &mut String) {
        generate_service(&service, output).expect("writing generated service code cannot fail");
    }
}

fn generate_service(service: &Service, output: &mut String) -> std::fmt::Result {
    let trait_name = &service.name;
    let client_name = format!("{}Client", service.name);
    let router_name = format!("{}Router", service.name);
    let methods_name = format!("{}_METHODS", screaming_snake(&service.name));
    let descriptor_name = format!("{}_DESCRIPTOR", screaming_snake(&service.name));
    let version = service
        .package
        .rsplit('.')
        .next()
        .unwrap_or(&service.package);

    writeln!(output, "pub trait {trait_name}: Send + Sync + 'static {{")?;
    for method in &service.methods {
        write_service_method(output, method)?;
    }
    writeln!(output, "}}")?;

    writeln!(output, "#[derive(Clone)]")?;
    writeln!(output, "pub struct {client_name} {{")?;
    writeln!(output, "    target: {client_name}Target,")?;
    writeln!(output, "}}")?;
    writeln!(output, "#[derive(Clone)]")?;
    writeln!(output, "enum {client_name}Target {{")?;
    writeln!(output, "    Direct(::std::sync::Arc<dyn {trait_name}>),")?;
    writeln!(
        output,
        "    Remote(::std::sync::Arc<dyn ::arut_rpc::RpcChannel>),"
    )?;
    writeln!(output, "}}")?;
    writeln!(output, "impl {client_name} {{")?;
    writeln!(
        output,
        "    pub fn direct(service: ::std::sync::Arc<dyn {trait_name}>) -> Self {{ Self {{ target: {client_name}Target::Direct(service) }} }}"
    )?;
    writeln!(
        output,
        "    pub fn remote(channel: ::std::sync::Arc<dyn ::arut_rpc::RpcChannel>) -> Self {{ Self {{ target: {client_name}Target::Remote(channel) }} }}"
    )?;
    for method in &service.methods {
        write_client_method(output, service, method, &client_name)?;
    }
    writeln!(output, "}}")?;

    writeln!(output, "pub struct {router_name}<T> {{")?;
    writeln!(output, "    service: ::std::sync::Arc<T>,")?;
    writeln!(output, "}}")?;
    writeln!(output, "impl<T> {router_name}<T> {{")?;
    writeln!(
        output,
        "    pub fn new(service: ::std::sync::Arc<T>) -> Self {{ Self {{ service }} }}"
    )?;
    writeln!(output, "}}")?;
    write_router_impl(output, service, &router_name, trait_name)?;

    writeln!(
        output,
        "pub const {methods_name}: &[::arut_rpc::MethodDescriptor] = &["
    )?;
    for method in &service.methods {
        let procedure = procedure(service, method);
        let kind = streaming_kind(method);
        writeln!(
            output,
            "    ::arut_rpc::MethodDescriptor {{ name: {:?}, procedure: {:?}, input: {:?}, output: {:?}, streaming: ::arut_rpc::StreamingKind::{kind} }},",
            method.proto_name, procedure, method.input_proto_type, method.output_proto_type
        )?;
    }
    writeln!(output, "];")?;
    writeln!(
        output,
        "pub const {descriptor_name}: ::arut_rpc::ServiceDescriptor = ::arut_rpc::ServiceDescriptor {{ name: {:?}, package: {:?}, version: {version:?}, methods: {methods_name} }};",
        service.proto_name, service.package
    )?;
    writeln!(
        output,
        "impl<T: {trait_name}> ::arut_rpc::RpcService for {router_name}<T> {{ fn descriptor(&self) -> &'static ::arut_rpc::ServiceDescriptor {{ &{descriptor_name} }} }}"
    )
}

fn write_service_method(output: &mut String, method: &Method) -> std::fmt::Result {
    let input = service_input(method);
    let output_type = service_output(method);
    writeln!(
        output,
        "    fn {}(&self, request: ::arut_rpc::Request<{input}>) -> ::arut_rpc::RpcFuture<::arut_rpc::Response<{output_type}>>;",
        method.name
    )
}

fn write_client_method(
    output: &mut String,
    service: &Service,
    method: &Method,
    client_name: &str,
) -> std::fmt::Result {
    let input = service_input(method);
    let output_type = service_output(method);
    let procedure = procedure(service, method);
    writeln!(
        output,
        "    pub fn {}(&self, request: ::arut_rpc::Request<{input}>) -> ::arut_rpc::RpcFuture<::arut_rpc::Response<{output_type}>> {{",
        method.name
    )?;
    writeln!(output, "        match &self.target {{")?;
    writeln!(
        output,
        "            {client_name}Target::Direct(service) => service.{}(request),",
        method.name
    )?;
    writeln!(
        output,
        "            {client_name}Target::Remote(channel) => {{"
    )?;
    writeln!(output, "                let channel = channel.clone();")?;
    write_remote_client_call(output, method, &procedure)?;
    writeln!(output, "            }}")?;
    writeln!(output, "        }}")?;
    writeln!(output, "    }}")
}

fn write_remote_client_call(
    output: &mut String,
    method: &Method,
    procedure: &str,
) -> std::fmt::Result {
    let shape = method_shape(method);
    let input = if method.client_streaming {
        "request.map(|stream| ::arut_rpc::map_stream(stream, |message| Ok(::arut_rpc::encode(message))))"
    } else {
        "request.map(::arut_rpc::encode)"
    };
    let output_type = &method.output_type;
    let decode = format!("|bytes| ::arut_rpc::decode::<{output_type}>(&bytes)");
    let result = if method.server_streaming {
        format!("Ok(response.map(|stream| ::arut_rpc::map_stream(stream, {decode})))")
    } else {
        format!("response.map({decode}).transpose()")
    };
    writeln!(
        output,
        "                Box::pin(async move {{ let response = channel.{shape}({procedure:?}, {input}).await?; {result} }})"
    )
}

fn write_router_impl(
    output: &mut String,
    service: &Service,
    router_name: &str,
    trait_name: &str,
) -> std::fmt::Result {
    writeln!(
        output,
        "impl<T: {trait_name}> ::arut_rpc::RpcChannel for {router_name}<T> {{"
    )?;
    for shape in ["unary", "server_stream", "client_stream", "bidirectional"] {
        write_router_method(output, service, shape)?;
    }
    writeln!(output, "}}")
}

fn write_router_method(output: &mut String, service: &Service, shape: &str) -> std::fmt::Result {
    let (request_type, response_type) = match shape {
        "unary" => (
            "::arut_rpc::Request<Vec<u8>>",
            "::arut_rpc::Response<Vec<u8>>",
        ),
        "server_stream" => (
            "::arut_rpc::Request<Vec<u8>>",
            "::arut_rpc::Response<::arut_rpc::RpcStream<Vec<u8>>>",
        ),
        "client_stream" => (
            "::arut_rpc::Request<::arut_rpc::RpcStream<Vec<u8>>>",
            "::arut_rpc::Response<Vec<u8>>",
        ),
        "bidirectional" => (
            "::arut_rpc::Request<::arut_rpc::RpcStream<Vec<u8>>>",
            "::arut_rpc::Response<::arut_rpc::RpcStream<Vec<u8>>>",
        ),
        _ => unreachable!(),
    };
    writeln!(
        output,
        "    fn {shape}(&self, procedure: &str, request: {request_type}) -> ::arut_rpc::RpcFuture<{response_type}> {{"
    )?;
    let methods = service
        .methods
        .iter()
        .filter(|method| method_shape(method) == shape)
        .collect::<Vec<_>>();
    if methods.is_empty() {
        writeln!(output, "        let _ = request;")?;
        writeln!(output, "        let procedure = procedure.to_owned();")?;
        writeln!(
            output,
            "        Box::pin(async move {{ Err(::arut_rpc::Status::unimplemented(&procedure)) }})"
        )?;
        return writeln!(output, "    }}");
    }
    writeln!(output, "        let _ = &request;")?;
    writeln!(output, "        match procedure {{")?;
    for method in methods {
        write_router_arm(output, service, method)?;
    }
    writeln!(
        output,
        "            _ => {{ let procedure = procedure.to_owned(); Box::pin(async move {{ Err(::arut_rpc::Status::unimplemented(&procedure)) }}) }}"
    )?;
    writeln!(output, "        }}")?;
    writeln!(output, "    }}")
}

fn write_router_arm(output: &mut String, service: &Service, method: &Method) -> std::fmt::Result {
    let procedure = procedure(service, method);
    let input = &method.input_type;
    let call = &method.name;
    writeln!(output, "            {procedure:?} => {{")?;
    writeln!(
        output,
        "                let service = self.service.clone();"
    )?;
    let decode = format!("|bytes| ::arut_rpc::decode::<{input}>(&bytes)");
    let request = if method.client_streaming {
        format!("request.map(|stream| ::arut_rpc::map_stream(stream, {decode}))")
    } else {
        format!("request.map({decode}).transpose()?")
    };
    let response = if method.server_streaming {
        "response.map(|stream| ::arut_rpc::map_stream(stream, |message| Ok(::arut_rpc::encode(message))))"
    } else {
        "response.map(::arut_rpc::encode)"
    };
    writeln!(
        output,
        "                Box::pin(async move {{ let request = {request}; let response = service.{call}(request).await?; Ok({response}) }})"
    )?;
    writeln!(output, "            }}")
}

fn service_input(method: &Method) -> String {
    if method.client_streaming {
        format!("::arut_rpc::RpcStream<{}>", method.input_type)
    } else {
        method.input_type.clone()
    }
}

fn service_output(method: &Method) -> String {
    if method.server_streaming {
        format!("::arut_rpc::RpcStream<{}>", method.output_type)
    } else {
        method.output_type.clone()
    }
}

fn method_shape(method: &Method) -> &'static str {
    match (method.client_streaming, method.server_streaming) {
        (false, false) => "unary",
        (false, true) => "server_stream",
        (true, false) => "client_stream",
        (true, true) => "bidirectional",
    }
}

fn streaming_kind(method: &Method) -> &'static str {
    match (method.client_streaming, method.server_streaming) {
        (false, false) => "Unary",
        (false, true) => "Server",
        (true, false) => "Client",
        (true, true) => "Bidirectional",
    }
}

fn procedure(service: &Service, method: &Method) -> String {
    format!(
        "/{}.{}/{}",
        service.package, service.proto_name, method.proto_name
    )
}

fn screaming_snake(name: &str) -> String {
    let mut result = String::new();
    for (index, character) in name.chars().enumerate() {
        if character.is_uppercase() && index != 0 {
            result.push('_');
        }
        result.push(character.to_ascii_uppercase());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost_build::Comments;

    #[test]
    fn discovers_proto_files_in_stable_order() {
        let root = std::env::temp_dir().join(format!("arut-protocol-build-{}", std::process::id()));
        let nested = root.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(root.join("z.proto"), "").unwrap();
        std::fs::write(nested.join("a.proto"), "").unwrap();
        std::fs::write(root.join("ignored.txt"), "").unwrap();

        let mut protos = Vec::new();
        collect_protos(&root, &mut protos).unwrap();
        protos.sort();

        assert_eq!(protos, vec![nested.join("a.proto"), root.join("z.proto")]);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn generates_every_rpc_streaming_shape() {
        let service = Service {
            name: "ShapeService".into(),
            proto_name: "ShapeService".into(),
            package: "arut.testing.v1".into(),
            comments: Comments::default(),
            methods: vec![
                method("unary", false, false),
                method("server", false, true),
                method("client", true, false),
                method("bidi", true, true),
            ],
            options: Default::default(),
        };
        let mut output = String::new();
        generate_service(&service, &mut output).unwrap();

        assert!(output.contains("channel.unary("));
        assert!(output.contains("channel.server_stream("));
        assert!(output.contains("channel.client_stream("));
        assert!(output.contains("channel.bidirectional("));
        assert!(output.contains("StreamingKind::Bidirectional"));
    }

    fn method(name: &str, client_streaming: bool, server_streaming: bool) -> Method {
        Method {
            name: name.into(),
            proto_name: name.into(),
            comments: Comments::default(),
            input_type: "Input".into(),
            output_type: "Output".into(),
            input_proto_type: ".arut.testing.v1.Input".into(),
            output_proto_type: ".arut.testing.v1.Output".into(),
            options: Default::default(),
            client_streaming,
            server_streaming,
        }
    }
}
