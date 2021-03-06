use rustbus::message_builder::MarshalledMessage;
use rustbus::params::message::Message;

#[derive(Debug)]
pub enum Error {
    RustbusError(rustbus::Error),
    DbusConnectionError(rustbus::client_conn::Error),
    DBusUnMashallError(rustbus::wire::unmarshal::Error),
    CouldNotConnectToDevice,
    CouldNotConnectToBus(String),
    UuidNotFound,
    DoesNotExist(Context),
    CharacteristicNotFound(Context),
    NoFdReturned,
    UnexpectedDbusReply,
    PairingTimeOut,
    CouldNotRemoveCache(std::io::Error),
    OperationNotSupported(Context),
    InvalidLength(Context),
    AuthenticationCanceled(Context),
    AuthenticationFailed(Context),
    BluezFailed(Context),
    NotPermitted(Context),
    InProgress(Context),
    UnknownErrorMessage(String),
}

impl PartialEq for Error {
    //check if same enum variant
    fn eq(&self, other: &Error) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::CouldNotRemoveCache(err)
    }
}

impl From<rustbus::wire::unmarshal::Error> for Error {
    fn from(err: rustbus::wire::unmarshal::Error) -> Error {
        Error::DBusUnMashallError(err)
    }
}

impl From<rustbus::client_conn::Error> for Error {
    fn from(err: rustbus::client_conn::Error) -> Error {
        Error::DbusConnectionError(err)
    }
}

impl From<rustbus::Error> for Error {
    fn from(err: rustbus::Error) -> Error {
        Error::RustbusError(err)
    }
}

impl From<(MarshalledMessage, Context)> for Error {
    fn from(err: (MarshalledMessage, Context)) -> Error {
        let (msg, context) = err;
        let msg = msg.unmarshall_all().unwrap();
        error_from(msg, context)
    }
}

impl<'a> From<(Message<'a, 'a>, Context)> for Error {
    fn from(err: (Message<'a, 'a>, Context)) -> Error {
        let (msg, context) = err;
        error_from(msg, context)
    }
}

#[derive(Debug, PartialEq)]
pub enum Context {
    Remove,
    Connect,
    Disconnect,
    Pair,
    StartDiscovery,
    StopDiscovery,
    AquireNotify(String),
    ReadValue(String),
    WriteValue(String),
}

fn unpack_msg(msg: &mut Message) -> Option<String> {
    let error_msg = msg.params.pop()?.into_string().ok()?;
    Some(error_msg)
}

pub fn error_from(mut msg: Message, context: Context) -> Error {
    if let Some(error_msg) = unpack_msg(&mut msg) {
        match error_msg.as_str() {
            "Operation is not supported" => return Error::OperationNotSupported(context),
            "Invalid Length" => return Error::InvalidLength(context),
            _ => (),
        }
    }
    if let Some(error_name) = &msg.dynheader.error_name {
        match error_name.as_str() {
            "org.bluez.Error.AuthenticationCanceled" => return Error::AuthenticationCanceled(context),
            "org.bluez.Error.AuthenticationFailed" => return Error::AuthenticationFailed(context),
            "org.bluez.Error.DoesNotExist" => return Error::DoesNotExist(context),
            "org.bluez.Error.Failed" => return Error::BluezFailed(context),
            "org.bluez.Error.NotPermitted" => return Error::NotPermitted(context),
            "org.bluez.Error.InProgress" => return Error::InProgress(context),
            _ => (),
        }
    }

    Error::UnknownErrorMessage(format!("{:?}", msg))
}
