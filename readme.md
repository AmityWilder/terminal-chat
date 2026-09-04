# Data Communications Semester Project

In this project you will be creating a chat application from scratch.

## Requirements

### Part 1

deadline: Sunday, October 11

- [x] Create a program with two components, a client and a server.
- [x] The two programs should be able to connect to each other and send at least one message each before the program terminates.
- The messages must
  - [x] be typed by the user
  - [x] both on the client side
  - [ ] and on the server side,
  - [x] transmitted through the socket
  - [x] and shown on the other end.
- [x] You must use socket programming, but there is no need to use threads or any other library.
- The program can terminate after the successful transmission of the message.
- There is no need to use two machines, you can run both programs on the same computer.

### Part 2

deadline: Sunday, November 8

- [x] Both the client and the server must remain online after the successful transmission of a message
- [x] and they must be able to continue to chat indefinitely.
- [x] The same person must be able to send multiple messages in a row.
- You are encouraged to use
  - [x] infinite loops
  - [ ] and multithreading.

### Part 3

deadline: Sunday, December 6

- [x] The server program is only used as a server to relay messages between clients.
- [x] A user cannot type a chat message on the server.
- [x] The server must be able to accept multiple clients.
- [x] The client program must be the same for all clients, that is, if you have 3 clients connecting to the server, do not write different code for each client. Simply run the same code multiple times.
- This is a group chat, i.e., each message is shown to all users who have connected to the server.

### Demo

At the last week of classes, each team must do a live demo of their project to the instructor (not in class). You cannot receive a grade for your project without a demo. All team members must be present.

### Bonus Points

- [ ] 10 Bonus points: Demo your project using at least two different machines.
- [x] 10 Bonus points: Ability to send a message to individual users (not just group chat).
- [x] 10 Bonus points: Ability to send any file.
