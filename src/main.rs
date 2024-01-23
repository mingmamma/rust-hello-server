use std::{net::{TcpListener, TcpStream}, io::{BufReader, BufRead, Write}, fs, thread, time::Duration};
use hello_rust_server::ThreadPool;


fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    let pool = ThreadPool::build(4).expect("Server setup error");

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        pool.execute(|| {
            handle_connection(stream);
        })
    }
}

fn handle_connection(mut stream: TcpStream) {
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
