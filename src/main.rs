use std::net::{TcpListener, TcpStream};
use std::io::{self, BufReader, BufRead, Write};
use std::fs;
use std::thread;
use std::time::Duration;
use hello_rust_server::ThreadPool;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    let pool = ThreadPool::build(4);

    for stream_result in listener.incoming() {
        let stream = stream_result.unwrap();
        pool.execute(|| {
            // respond_by_request_line(stream);
            respond_by_request_verbatim_print(stream);
        })
    }
}

fn respond_generic_page_by_request_line(mut stream: TcpStream) {
    let buf_reader = BufReader::new(&mut stream);
    
    // Collecting the http request into a vector and print the request info 
    // let http_requests: Vec<_> = buf_reader.lines()
    // .map(|result| result.unwrap() )
    // // The browser signals the end of an HTTP request by sending two newline characters in a row, 
    // // so to get one request from the stream, 
    // // we take lines until we get a line that is the empty string
    // .take_while(|line| !line.is_empty())
    // .collect();
    // println!("Request: {:#?}", http_requests);
    
    let request_line = buf_reader.lines().next().unwrap().unwrap();

    let (status_line, filename) = match request_line.as_str() {
        "GET / HTTP/1.1" => ("HTTP/1.1 200 OK", "hello.html"),
        "GET /sleep HTTP/1.1" => {
            thread::sleep(Duration::from_secs(5));
            ("HTTP/1.1 200 OK", "hello.html")
        },
        _ => ("HTTP/1.1 404 Not Found", "404.html")
    };

    let contents = fs::read_to_string(filename).unwrap();
    let length = contents.len();
    let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}");


    stream.write_all(response.as_bytes()).unwrap();                    
}

fn respond_by_request_verbatim_print(mut stream: TcpStream) {
    let buf_reader: BufReader<&mut TcpStream> = BufReader::new(&mut stream);
    
    // desugared & annotated equivalent implementation
    // let lines: Lines<BufReader<&mut TcpStream>> = buf_reader
    //             .lines();
    // let mapped_lines = lines.map(|line: Result<String, Error>| -> String 
    //     { line.unwrap() });
    // let http_request: Vec<String> = mapped_lines
    //                                 .take_while(|line| !line.is_empty())
    //                                 .collect();
    
    let http_request: Vec<String> = buf_reader
        .lines()
        .map(|result| result.unwrap())
        .take_while(|line| !line.is_empty())
        .collect();

    println!("Request: {:#?}", http_request);
}
