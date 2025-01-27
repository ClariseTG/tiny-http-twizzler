// assigns dynamic ports as needed
// dynamic ports range from 49152 to 65535 (size of stack = 16383)

/* what this file should do:
 * create a stack for the dynamic port numbers
 * support stack operations as follows:
 *    pop(): pop off the next useable port
 *    push(): push a port to be used next
 * create a wrapper for the stack in a mutex.
 * the function get_ephemeral_port() will pop off the stack
 */
use std::sync::{Arc, Mutex};
pub struct PortAssigner {
  stack: Arc<Mutex<Stack>>,
}
struct Stack {
  stack: Vec<u16>,
}

impl Stack {
  pub fn new() -> Self {
    let mut stack = Vec::with_capacity(16384);
    for i in 0..=16383 { // capacity of 16384
      stack.push(65535-i);
    }
    Self {stack: stack}
  }
  pub fn pop(&mut self) -> Option<u16> {
    self.stack.pop()
  }
  pub fn push(&mut self, port: u16) {
    self.stack.push(port)
  }
}

impl PortAssigner {
  pub fn new() -> Self {
    Self {
      stack: Arc::new(Mutex::new(Stack::new()))
    }
  }
  pub fn return_port(&self, port: u16) {
    // pushes port to stack
    self.stack.lock().unwrap().push(port)
  }
  pub fn get_ephemeral_port(&self) -> Option<u16> {
    // pops port off of stack
    self.stack.lock().unwrap().pop()
  }
}


// todo:
// implement backlog for accept(): have 8 waiting listening sockets before the indiana jones swap <-- this is for me
// look at daniel's changes in code <-- daniel moved swap into the blocking code
// test with 100 simultaneous connections!!


#[cfg(test)]
mod tests {
  use crate::port::PortAssigner;
  #[test]
  fn allocate_port() {
    let mut p = PortAssigner::new();
    if let Some(port) = p.get_ephemeral_port() {
      println!("{}", port);
      p.return_port(port);
    } else {
      println!("none");
    }
    assert_eq!(p.get_ephemeral_port(), Some(49152));
  }
  #[test]
  fn get_last_port() {
    let mut p = PortAssigner::new();
    let mut port = p.get_ephemeral_port();
    while port != None {
      port = p.get_ephemeral_port();
    }
    port = p.get_ephemeral_port(); // returns None
  }
}