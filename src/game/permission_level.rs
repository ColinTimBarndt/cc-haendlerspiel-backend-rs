use num_derive::FromPrimitive;

#[derive(Clone, Copy, FromPrimitive, Eq, PartialEq, Ord, PartialOrd)]
pub enum PermissionLevel {
  Guest = 0,
  Moderator = 1,
  Admin = 2,
}
