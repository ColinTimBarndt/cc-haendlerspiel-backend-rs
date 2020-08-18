# Network Traffic Protocol
The Haendlerspiel-Protocol (HSP) uses TCP for network communication.
Each packet is encoded using the following data types:

| Type | Bytes | Description                       |
| ---- | ----: | --------------------------------- |
| u8   | 1     | Unsigned Integer                  |
| u16  | 2     | Unsigned Integer                  |
| u32  | 4     | Unsigned Integer                  |
| u64  | 8     | Unsigned Integer                  |
| i8   | 1     | Signed Integer                    |
| i16  | 2     | Signed Integer                    |
| i32  | 4     | Signed Integer                    |
| i64  | 8     | Signed Integer                    |
| f32  | 4     | Single-Precision Floating point   |
| f64  | 8     | Double-Precision Floating point   |

## Packet format
| Type | Description                   |
| ---- | ----------------------------- |
|      | **Packet Header**             |
| u16  | Packet type identifier        |
| u32  | Packet body length (bytes)    |
|      | **Packet Body**                |
| *    | Depends on the packet type    |