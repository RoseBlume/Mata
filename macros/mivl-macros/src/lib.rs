//! No syn/quote: #[mivl::command] exports a same-name registrar module.
//! The `foo` module lives in Rust's type namespace; the function `foo` lives
//! in the value namespace. Thus `foo(...)` and `foo::register(...)` coexist.
use proc_macro::{Delimiter, TokenStream, TokenTree};
use std::str::FromStr;

fn error(message:&str)->TokenStream {
    format!("compile_error!({message:?});").parse().expect("error tokens")
}
#[proc_macro_attribute]
pub fn command(attr:TokenStream,item:TokenStream)->TokenStream {
    if !attr.is_empty(){return error("#[command] does not accept arguments");}
    match expand(item){Ok(tokens)=>tokens,Err(message)=>error(&message)}
}
fn expand(item:TokenStream)->Result<TokenStream,String> {
    let tokens:Vec<TokenTree>=item.clone().into_iter().collect();
    let fn_pos=tokens.iter().position(|t|matches!(t,TokenTree::Ident(x) if x.to_string()=="fn"))
        .ok_or("#[command] must annotate a free function")?;
    let head=&tokens[..fn_pos];
    if head.iter().any(|t|matches!(t,TokenTree::Ident(x) if matches!(x.to_string().as_str(),"const"|"unsafe"|"extern"))) {
        return Err("#[command] accepts safe, non-const functions".into());
    }
    let is_async=head.iter().any(|t|matches!(t,TokenTree::Ident(x) if x.to_string()=="async"));
    let function=match tokens.get(fn_pos+1){Some(TokenTree::Ident(x))=>x.to_string(),_=>return Err("expected function name".into())};
    let group=match tokens.get(fn_pos+2){Some(TokenTree::Group(g)) if g.delimiter()==Delimiter::Parenthesis=>g,_=>return Err("generic commands or methods are not supported".into())};
    // Split only top-level commas: nested generic angle brackets are deliberately
    // not supported, while Vec<Value> is supported by FromCommandArg.
    let mut params:Vec<String>=Vec::new();
    let mut chunk:Vec<TokenTree>=Vec::new();
    for tok in group.stream().into_iter().chain(std::iter::once(TokenTree::Punct(proc_macro::Punct::new(',',proc_macro::Spacing::Alone)))) {
        if matches!(&tok,TokenTree::Punct(p) if p.as_char()==',') {
            if !chunk.is_empty() {
                let valid=matches!(chunk.first(),Some(TokenTree::Ident(id)) if id.to_string()!="self")
                    && matches!(chunk.get(1),Some(TokenTree::Punct(p)) if p.as_char()==':');
                if !valid{return Err("command parameters must be named owned arguments".into());}
                if chunk[2..].iter().any(|t|matches!(t,TokenTree::Punct(p) if matches!(p.as_char(),'&'|'*'))) {
                    return Err("reference/pointer command parameters are unsupported".into());
                }
                params.push(chunk[2..].iter().cloned().collect::<TokenStream>().to_string());
                chunk.clear();
            }
        } else {chunk.push(tok);}
    }
    if params.len()>32{return Err("commands support at most 32 arguments".into());}
    if !matches!(tokens.last(),Some(TokenTree::Group(g)) if g.delimiter()==Delimiter::Brace) {
        return Err("command requires a body".into());
    }
    // For a module path resources::add_numbers the command's invocation name
    // is `add_numbers`, and its registrar is `resources::add_numbers::register`.
    let arity=params.len();
    let arguments=params.iter().enumerate().map(|(i,ty)|format!(
        "let arg_{i}: {ty} = ::mivl::FromCommandArg::from_command_arg(&args[{i}])?;"
    )).collect::<Vec<_>>().join("\n");
    let args=(0..arity).map(|i|format!("arg_{i}")).collect::<Vec<_>>().join(",");
    let body=if is_async {
        format!(r#"
            engine.register_async_command("{function}", |args: &[::mivl::Value]| {{
                if args.len()!={arity} {{return Err(::mivl::Error::command(format!("command '{function}' expects {arity} arguments, received {{}}", args.len())));}}
                {arguments}
                let future: ::mivl::CommandFuture = Box::pin(async move {{
                    ::mivl::IntoCommandResult::into_command_result(super::{function}({args}).await)
                }});
                Ok(future)
            }})
        "#)
    } else {
        format!(r#"
            engine.register_command("{function}", |args: &[::mivl::Value]| {{
                if args.len()!={arity} {{return Err(::mivl::Error::command(format!("command '{function}' expects {arity} arguments, received {{}}", args.len())));}}
                {arguments}
                ::mivl::IntoCommandResult::into_command_result(super::{function}({args}))
            }})
        "#)
    };
    let generated=format!(r#"
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub mod {function} {{
            pub fn register(engine: &mut ::mivl::Engine) -> ::mivl::Result<()> {{ {body} }}
        }}
        #[doc = "Backward-compatible explicit registration helper."]
        pub fn register_{function}(engine: &mut ::mivl::Engine) -> ::mivl::Result<()> {{
            {function}::register(engine)
        }}
    "#);
    let expanded=TokenStream::from_str(&generated).map_err(|e|format!("cannot generate command registrar: {e}"))?;
    Ok(item.into_iter().chain(expanded).collect())
}
