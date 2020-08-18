# Network Traffic Protocol

The Haendlerspiel-Protocol (HSP) uses TCP for network communication.
Packets are sent over the streams using the following encoding specification.
If there is any error in the connection, the connection is immediately shut
down.

## Data Types

Each packet is encoded using the following data types:

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

| Type  | Description                                             |
| ----- | ------------------------------------------------------- |
|       | **Packet Header**                                       |
| `u16` | Packet type identifier                                  |
| `u32` | Packet body length (bytes)                              |
|       | **Packet Body**                                         |
| ?     | Depends on the [packet type](#Packet%20Types) and state |

## Packet Types

Here is a list of all possible packet types and the state they are bound to.
Each connection has the initial state `Handshake`.

| State     |  ID | Bound to | Packet Documentation                                   |
| --------- | --: | -------- | ------------------------------------------------------ |
| Handshake |   0 | Server   | [Handshake](#Handshake%20Packet)                       |
| Ping      |   0 | Client   | [Ping Status](#Ping%20Status%20Packet)                 |
| Ping      |   1 | Both     | [Ping Pong](#Ping%20Pong%20Packet)                     |
| Encrypt   |   0 | Client   | [Request Encryption](#Request%20Encryption%20Packet)   |
| Encrypt   |   0 | Server   | [Encryption Response](#Encryption%20Response%20Packet) |

### Handshake Packet

| Type | Description |
| ---- | ----------- |
| `u8` | Action enum |

Possible values for the next state are:

- 1: Changes the connection state to `Ping` and sends a
  [Ping Status](#Ping%20Status%20Packet) Packet.
- 2: Changes the connection state to `Encrypt` and sends a
  [Request Encryption](#Request%20Encryption%20Packet) Packet.

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

### Request Encryption Packet

| Type      | Description                   |
| --------- | ----------------------------- |
| `u32`     | Length of public RSA key DER  |
| `u8` \* n | Public RSA key encoded in DER |
| `u32`     | Length of verification key    |
| `u8` \* n | Verification key              |

The server sends this packet when enabling encryption. The cipher used
is `AES 128 CFB8`. The client has to respond to this packet with an
[Encryption Response](#Encryption%20Response) Packet.

### Encryption Response Packet

| Type      | Description                |
| --------- | -------------------------- |
| `u32`     | Length of verification key |
| `u8` \* n | Encrypted verification key |
| `u32`     | Length of shared secret    |
| `u8` \* n | Encrypted shared secret    |

The client response this packet to activate `AES 128 CFB8` encryption.
To verify that the public key was transferred correctly, the verification
key has to be encrypted using it. The client also has to generate a
shared secret (which should be 128 bytes long) and encrypt it using the
same public key.

If the encryption fails, the server immediately terminates the connection.
If it succeeds, any bytes sent after this packet are encrypted using the
cipher and the state is set to `Login`. The server will respond with an
[Encryption Success](#Encryption%20Success%20Packet) Packet.

Note that the cipher is only updated and never reset for the next packet.

### Encryption Success Packet

| Type  | Description         |
| ----- | ------------------- |
| `u32` | Always `0xDEADBEEF` |
