# Terminal Chat

An application for sending messages between different terminals.

## How to use

### Step 1

Run the **server** program in any terminal.

- Windows 10: [`server_win10.exe`](./server_win10.exe)
- Mac (OSX): [`server_osx`](./server_osx)
- Linux: (not yet supported)

If you would like to host on a specific socket address, provide that address as a command line argument. The format must use `.` to separate quartets, and `:` to separate the IP from the port. Both IPv4 and IPv6 are supported (however only IPv4 has been tested). Choosing 0 for the port will allow a port to be chosen for you.

**Example:** `server_win10.exe 127.0.0.1:8080`

Note: This particular example happens to also be the default socket address if no command line arguments are provided.

Upon successful initialization, the appliction will output the following:

    hosting on <the requested socket address> (<the socket address being listened on>)
    awaiting client connect...

### Step 2

Run the **client** program in any other terminal.

- Windows 10: [`client_win10.exe`](./client_win10.exe)
- Mac (OSX): [`client_osx`](./client_osx)
- Linux: (not yet supported)

By default, the application will try to connect to the socket address `127.0.0.1:8080`. If you did not provide any command line arguments to the server, this should work fine as that's the default the server will host on. Otherwise, you will need to provide a socket address to connect to.

To connect to a specific socket, provide `<the socket address being listened on>` (from [step 1](#step-1)) as a command line argument.

**Example:** If [step 1](#step-1) had the following output:

    hosting on 127.0.0.1:0 (127.0.0.1:61495)
    awaiting client connect...

You should run `client_win10.exe 127.0.0.1:61495`

After running the client program, it will attempt to connect for up to 2 seconds. If 2 seconds pass without successfully connecting to the server, it will give up and end the process on its own. The `--timeout` or `-t` command line argument can be used to provide a custom timeout. Note, however, that the application will block until a connection is made or the timeout ends, so you will have to use `ctrl+c` to terminate the program early if you gave an absurdly long timeout on a socket that doesn't actually have a server.

### Step 3

#### Messages

To send a message: While in a terminal running the client program (after successfully connecting to a server), simply type some text that you want to send. When you press enter, nothing will happen. You may continue writing additional lines of text or enter a [command](#commands). You may continue adding text to a message after entering a command. The message will not be sent until you press enter on an empty line (whitespace is not considered empty, and may be used to double-space lines).

By default, your message will be sent to the global chat. All connected clients will receive your message, however it will not be buffered for disconnected clients.

#### Commands

To enter a command: At the start of a new line, press `/`, then enter the name of the command you want to use, followed by the arguments for the command. To see a list of commands and what they do, enter `/help`.

Some particularly useful ones are:

- `/atch.add` - Add an attachment to the current message. Up to 8 attachments can be added. The attachment must be a file on your computer, which you must reference by filesystem path. An attachment can be any file. Attachments may include alt text to describe them. Alt text can be a double-quoted (`"`) string containing escape characers. The supported escapes are:
  - `\x##` (replace `#` with hexadecimal digits (0-9, a-f, A-F)): the hexadecimal value as a byte
  - `\o###` (replace `#` with octal digits (0-7)): the octal value as a byte
  - `\b########` (replace `#` with binary digits (0-1)): the binary value as a byte
  - `\0`-`\9`: the decimal value as a byte
  - `\a`: bell
  - `\b`: backspace
  - `\f`: form feed
  - `\n`: newline
  - `\r`: return carriage
  - `\t`: tab
  - `\v`: vertical tab
  - `\\`: `\`
  - `\"`: `"`
  - `\'`: `'`

- `/atch.save` - Download an attachment, by filename, from the most recent loaded message.

- `/iam` - Give yourself a username. Usernames have character/length requirements that will be presented if you enter an invalid one. If the name is not in use, you may make up a password. If it *is* in use, you must enter the password that is associated with that username (or choose a different username). The password has no security requirements. User identities will persist as long as the server program is running, but not once it is shut down. Multiple client programs are allowed to use the same username, as long as all of them log in with the same password.

- `/chat.new` - Create a new chat with a name (first argument) and members (all following arguments). Chat members can be either sockets or user names, and do not need to be prefixed with anything (prefixes are used to distinguish usernames from chat names. There is no such need for distinguishing usernames from socket addresses, as usernames are not allowed to start with a number).

- `/chat.set` - Connect to a chat or user by name. If you know a user's socket address, you may connect to that directly (there is no benefit to doing this except that you can connect to a client that does not have a username). When connected to a chat, all messages you send until you connect to a different chat will be sent to every member of the chat, and only members of the chat. It will also load the last 10 messages sent from other users.
  - To connect to a chat, you must prefix its name with `#`.
  - To connect to a user by name, you must prefix their username with `@`.
  - To connect to a socket address, do not prefix it with anything.

Remember that you can use `/help` for a full list and specifics on how to run the commands.

### Step 4

To end either a server or client side of the application, type `/exit` and press enter. This will tell the program to shutdown cleanly.
You may prefer to do this on the server side, as all connected clients will automatically exit when the server hosting them stops running.
