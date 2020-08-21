# Network Traffic Protocol

The Haendlerspiel-Protocol (HSP) uses TCP with TLS encryption for network communication.
Packets are serialized using the following specification.
If there is any error in the connection, the connection is immediately shut
down.

## Data Types

Each packet is encoded using the following data types. Note that all numeric types
are using LITTLE ENDIAN byte order and not big endian. This choice was made because
LE is the native byte order of most modern processors.

| Type   | Bytes | Description                     |
| ------ | ----: | ------------------------------- |
| `u8`   |     1 | Unsigned Integer                |
| `u16`  |     2 | Unsigned Integer                |
| `u32`  |     4 | Unsigned Integer                |
| `u64`  |     8 | Unsigned Integer                |
| `i8`   |     1 | Signed Integer                  |
| `i16`  |     2 | Signed Integer                  |
| `i32`  |     4 | Signed Integer                  |
| `i64`  |     8 | Signed Integer                  |
| `f32`  |     4 | Single-Precision Floating point |
| `f64`  |     8 | Double-Precision Floating point |
| `str`  |   4+n | [UTF-8 String](#Strings)        |
| `name` |   1+n | [UTF-8 String](#Strings)        |

### Strings

#### str

| Type      | Description     |
| --------- | --------------- |
| `u32`     | Length in bytes |
| `u8` \* n | UTF-8 data      |

#### name

| Type      | Description     |
| --------- | --------------- |
| `u8`      | Length in bytes |
| `u8` \* n | UTF-8 data      |

## Packet format

| Type  | Description                                           |
| ----- | ----------------------------------------------------- |
|       | **Packet Header**                                     |
| `u16` | Packet type identifier                                |
| `u32` | Packet body length (bytes)                            |
|       | **Packet Body**                                       |
| ?     | Depends on the [packet type](#Packet-Types) and state |

## Packet Types

Here is a list of all possible packet types and the state they are bound to.
Each connection has the initial state `Handshake`.

| State         |  ID | Bound to | Documentation                            |
| ------------- | --: | -------- | ---------------------------------------- |
| **Handshake** |     |          |                                          |
| Handshake     |   0 | Server   | [Handshake](#Handshake-Packet)           |
| **Ping**      |     |          |                                          |
| Ping          |   0 | Client   | [Ping Status](#Ping-Status-Packet)       |
| Ping          |   1 | Both     | [Ping Pong](#Ping-Pong-Packet)           |
| **Login**     |     |          |                                          |
| Login         |   0 | Client   | [List Games](#List-Games-Packet)         |
| Login         |   0 | Server   | [Sync Games](#Sync-Games-Packet)         |
| Login         |   1 | Server   | [Login](#Login-Packet)                   |
| Login         |   1 | Client   | [Login Response](#Login-Response-Packet) |

### Handshake Packet

| Type | Description |
| ---- | ----------- |
| `u8` | Action enum |

Possible values for the next state are:

- 1: Changes the connection state to `Ping` and sends a
  [Ping Status](#Ping-Status-Packet) Packet.
- 2: Changes the connection state to `Login` and sends a
  [List Games](#List-Games-Packet) Packet.

### Ping Status Packet

| Type  | Description               |
| ----- | ------------------------- |
| `u32` | Total player count        |
| `u32` | Total running games count |
| `str` | Status message as JSON    |

### Ping Pong Packet

| Type  | Description   |
| ----- | ------------- |
| `u64` | Random number |

The server is going to send this packet back to the sender
without any changes and then closes the connection if received.
The client may use this behavior to measure the connection speed
to the server. If the client does not want to do this, it can
just close the connection.

### List Games Packet

| Type       | Description                               |
| ---------- | ----------------------------------------- |
| `u32`      | Number of entries                         |
| Entry \* n | [Entry Data](#List-Games-Entry-Data-Type) |

Sent when entering `Login`-state and every time the list changes.
If a game entry is removed that the client never had registered, then
the client should ignore it and send a [Sync Games](#Sync-Games-Packet)
Packet to notify the server that it should re-send all entries.

#### List Games Entry Data Type

| Type   | Description            |
| ------ | ---------------------- |
| `u64`  | Identifier of the game |
| `u8`   | 0: Add, 1: Remove      |
|        | **If Add**             |
| `name` | Name of the game       |
| `u32`  | Player count           |

### Sync Games Packet

| Type | Description    |
| ---- | -------------- |
|      | _Empty packet_ |

The client should send this packet if the server unregisteres a
game that the client did not know existed. It tells the server
to re-send all existing games.

### Login Packet

| Type   | Description |
| ------ | ----------- |
| `name` | Username    |
| `name` | Password    |

Log in as an existing user. Only logged in users are able to create
game instances. A user can log out by sending empty strings for
both fields. Every user that is not logged in is a `Guest`.

### Login Response Packet

| Type | Description      |
| ---- | ---------------- |
| `u8` | Permission level |

The server response to a login by sending the users new permission
level (0: Guest, 1: Moderator, 2: Administrator).
If the level is 0, the login information was false.

In case both username and password were empty, a permission level
response of 0 indicates that the user has successfully logged off.
