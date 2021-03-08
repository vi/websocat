
use std::str::FromStr;


#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut from_str_mode = false;

    let mut treestrings = vec![];
    let mut program_name_processed = false;

    for arg in std::env::args_os() {
        if !program_name_processed {
            program_name_processed = true;
            continue;
        }
        if arg == "--str" {
            from_str_mode = true;
        } else if from_str_mode {
            let s = arg.to_str().unwrap();

            match websocat_api::StrNode::from_str(s) {
                Ok(x) => println!("{}", x),
                Err(e) => println!("{:#}", e),
            }
        } else {
            let s = arg.to_str().unwrap().to_owned();
            treestrings.push(s);
        }
    }

    if treestrings.len() != 2 {
        panic!("Exactly two positional arguments requires");
    }

    tracing_subscriber::fmt::init();

    let reg = websocat_allnodes::all_node_classes();

    let c = websocat_api::Session::build_from_two_tree_strings(
        &reg, 
        &treestrings[0],
        &treestrings[1],
    ).unwrap();

    println!("{}", websocat_api::StrNode::reverse(c.left, &c.nodes).unwrap());
    println!("{}", websocat_api::StrNode::reverse(c.right, &c.nodes).unwrap());
    

    if let Err(e) = websocat_session::run(c).await {
        eprintln!("Error: {:#}", e);
    }


}

