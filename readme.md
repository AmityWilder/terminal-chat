# Terminal Chat

An application for sending messages between different terminals.

## How to use

### Step 1

Run the **server** program in any terminal.

- Windows 10: [`server_win10.exe`](./server_win10.exe)
- Mac (OSX): [`server_osx.???`](./server_osx.???)
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
- Mac (OSX): [`client_osx.???`](./client_osx.???)
- Linux: (not yet supported)

By default, the application will try to connect to the socket address `127.0.0.1:8080`. If you did not provide any command line arguments to the server, this should work fine as that's the default the server will host on. Otherwise, you will need to provide a socket address to connect to.

To connect to a specific socket, provide `<the socket address being listened on>` (from [step 1](#step-1)) as a command line argument.

**Example:** If [step 1](#step-1) had the following output:

    hosting on 127.0.0.1:0 (127.0.0.1:61495)
    awaiting client connect...

You should run `client_win10.exe 127.0.0.1:61495`

After running the client program, it will attempt to connect for up to 2 seconds. If 2 seconds pass without successfully connecting to the server, it will give up and end on its own.

### Step 3
