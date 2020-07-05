use std::fs::File;
use std::os::unix::io::FromRawFd;
use std::time::Duration;
use std::collections::HashMap;

use rustbus::client_conn::Timeout;
use rustbus::params::message;
use rustbus::{params, get_system_bus_path, standard_messages, Conn, MessageBuilder, RpcConn};
mod error;
mod experiments;
use error::{to_error, Error, ErrorContext};
mod dbus_helpers;
use dbus_helpers::{
    unwrap_base, unwrap_bool, 
    unwrap_container, unwrap_dict, 
    unwrap_objectpath, unwrap_string,
    unwrap_variant, get_name_owner, register_agent,
};
//idea:
// -simple, no auth or pairing supported
// no need to explicitly connect
// builder pattern, by default pick first adapter (other is extra feature)
// use "from_raw_fd" for File (write and notify)
// store opened files in self so we close them on drop
// builder pattern for all operations

// extra ideas :
// (safety) disconnect all connected on drop [make builder option?]

const TIMEOUT: Timeout = Timeout::Duration(Duration::from_secs(5));

pub struct BleBuilder {
    connection: RpcConn,
    adapter_numb: u8,
    conn_name: String
}

impl BleBuilder {
    pub fn new() -> Result<Self, Error> {
        let session_path = get_system_bus_path()?;
        let con = Conn::connect_to_bus(session_path, true)?;
        let mut connection = RpcConn::new(con);
        // send the obligatory hello message
        let response_serial = connection.send_message(&mut standard_messages::hello(), Timeout::Infinite)?;
        let mut reply = connection.wait_response(response_serial, TIMEOUT)?.unmarshall_all()?;
        let param = reply.params.pop().unwrap();
        let container = unwrap_base(param).unwrap();
        let conn_name = unwrap_string(container).unwrap();
        
        dbg!(&conn_name);

        Ok(BleBuilder {
            conn_name,
            connection,
            adapter_numb: 0,
        })
    }

    pub fn build(mut self) -> Result<Ble, Error> {
        let mut message = get_name_owner("org.bluez".to_owned())?;
        let response_serial = self
            .connection
            .send_message(&mut message, TIMEOUT)?;
        let msg = self.connection
            .wait_response(response_serial, TIMEOUT)?
            .unmarshall_all()?;
        dbg!(msg);

        /*let mut message = request_name("/test/hoi".to_owned(), DBUS_NAME_FLAG_REPLACE_EXISTING);
        let response_serial = self
            .connection
            .send_message(&mut message, TIMEOUT)?;
        let msg = self.connection.wait_response(response_serial, TIMEOUT)?;
        dbg!(msg);*/

        let mut message = register_agent("/test/hoi", "KeyboardDisplay")?;
        let response_serial = self
            .connection
            .send_message(&mut message, TIMEOUT)?;
        let msg = self.connection
            .wait_response(response_serial, TIMEOUT)?
            .unmarshall_all()?;

        //let mut message = 
        dbg!(msg);

        let BleBuilder {
            conn_name,
            connection,
            adapter_numb,
        } = self;

        Ok(Ble {
            connection,
            adapter_numb,
        })
    }
}

pub struct Ble {
    //adapter
    connection: RpcConn,
    adapter_numb: u8,
}

impl Ble {
    #[allow(dead_code)]
    pub fn connect(&mut self, adress: impl Into<String>) -> Result<(), Error> {
        let adress = adress.into().replace(":", "_");

        let mut connect = MessageBuilder::new()
            .call("Connect".into())
            .at("org.bluez".into())
            .on(format!(
                "/org/bluez/hci{}/dev_{}",
                self.adapter_numb, adress
            ))
            .with_interface("org.bluez.Device1".into()) //is always Device1
            .build();

        let response_serial = self.connection.send_message(&mut connect, TIMEOUT)?;
        let msg = self.connection.wait_response(response_serial, TIMEOUT)?;

        match msg.typ {
            rustbus::MessageType::Reply => Ok(()),
            rustbus::MessageType::Error => Err(Error::from(msg)),
            _ => {
                let dbg_str = format!(
                    "Unexpected Dbus message, Connect should only 
                    be awnserd with Error or Reply however we got: {:?}",
                    &msg
                );
                dbg!(&dbg_str);
                panic!();
            }
        }
    }

    #[allow(dead_code)]
    pub fn disconnect(&mut self, adress: impl Into<String>) -> Result<(), Error> {
        let adress = adress.into().replace(":", "_");

        let mut connect = MessageBuilder::new()
            .call("Disconnect".into())
            .at("org.bluez".into())
            .on(format!(
                "/org/bluez/hci{}/dev_{}",
                self.adapter_numb, adress
            ))
            .with_interface("org.bluez.Device1".into()) //is always Device1
            .build();

        let response_serial = self.connection.send_message(&mut connect, TIMEOUT)?;
        let msg = self.connection.wait_response(response_serial, TIMEOUT)?;

        match msg.typ {
            rustbus::MessageType::Reply => Ok(()),
            rustbus::MessageType::Error => Err(Error::from(msg)),
            _ => {
                let dbg_str = format!(
                    "Connect can only be awnserd 
                    with Error or Reply however we got: {:?}",
                    &msg
                );
                dbg!(&dbg_str);
                panic!();
            }
        }
    }

    #[allow(dead_code)]
    pub fn is_connected(&mut self, adress: impl Into<String>) -> Result<bool, Error> {
        let adress = adress.into().replace(":", "_");
        let mut is_connected = MessageBuilder::new()
            .call("Get".into())
            .at("org.bluez".into())
            .on(format!(
                "/org/bluez/hci{}/dev_{}",
                self.adapter_numb, &adress
            ))
            .with_interface("org.freedesktop.DBus.Properties".into())
            .build();
        is_connected.body.push_param("org.bluez.Device1")?;
        is_connected.body.push_param("Connected")?;

        let response_serial = self.connection.send_message(&mut is_connected, TIMEOUT)?;
        let mut reply = self.connection
            .wait_response(response_serial, TIMEOUT)?
            .unmarshall_all()?;

        let param = reply.params.pop().unwrap();
        let container = unwrap_container(param).unwrap();
        let variant = unwrap_variant(container).unwrap();
        let param = variant.value;
        let base = unwrap_base(param).unwrap();
        let connected = unwrap_bool(base).unwrap();

        Ok(connected)
    }

    #[allow(dead_code)]
    pub fn notify(
        &mut self,
        adress: impl Into<String>,
        uuid: impl AsRef<str>,
    ) -> Result<File, Error> {
        let char_path = self
            .path_for_char(adress, uuid)?
            .ok_or(Error::CharacteristicNotFound)?;

        let mut aquire_notify = MessageBuilder::new()
            .call("AcquireNotify".into())
            .at("org.bluez".into())
            .on(char_path.clone())
            .with_interface("org.bluez.GattCharacteristic1".into()) //is always GattCharacteristic1
            .build();
        
        
        let dic = params::Dict {
            key_sig: rustbus::signature::Base::String, 
            value_sig: rustbus::signature::Type::Container(rustbus::signature::Container::Variant), 
            map: HashMap::new()
        };
        let dic = rustbus::params::Container::Dict(dic);
        let param = rustbus::params::Param::Container(dic);
        //let dict = params::Container::Dict()
        //let test = HashMap::new::<params::Base::String, params::Variant>();
        aquire_notify.body.push_old_param(&param)?;
        //aquire_notify.body.push_param(dic);
        dbg!(&aquire_notify);

        let response_serial = self.connection.send_message(&mut aquire_notify, TIMEOUT)?;
        let reply = self.connection
            .wait_response(response_serial, TIMEOUT)?
            .unmarshall_all()?;
        dbg!(&reply);

        match &reply.typ {
            rustbus::MessageType::Error => {
                return Err(to_error(reply, ErrorContext::AquireNotify(char_path)))
            }
            rustbus::MessageType::Reply => (),
            _ => Err(Error::UnexpectedDbusReply)?,
        }

        let message::Message { mut raw_fds, .. } = reply;
        let raw_fd = raw_fds.pop().ok_or(Error::NoFdReturned)?;
        let file = unsafe { File::from_raw_fd(raw_fd) };
        Ok(file)
    }

    fn path_for_char(
        &mut self,
        adress: impl Into<String>,
        char_uuid: impl AsRef<str>,
    ) -> Result<Option<String>, Error> {
        let mut get_paths = MessageBuilder::new()
            .call("GetManagedObjects".into())
            .at("org.bluez".into())
            .on("/".into())
            .with_interface("org.freedesktop.DBus.ObjectManager".into())
            .build();

        let response_serial = self.connection.send_message(&mut get_paths, TIMEOUT)?;
        let mut reply = self.connection
            .wait_response(response_serial, TIMEOUT)?
            .unmarshall_all()?;

        let param = reply.params.pop().unwrap();
        let container = unwrap_container(param).unwrap();
        let dict = unwrap_dict(container).unwrap();

        let device_path = format!(
            "/org/bluez/hci{}/dev_{}",
            self.adapter_numb,
            adress.into().replace(":", "_")
        );

        for (path, base) in dict
            .into_iter()
            .filter_map(unwrap_objectpath)
            .filter(|(p, _)| p.contains(&device_path))
            .filter(|(p, _)| p.contains("char"))
            .filter(|(p, _)| !p.contains("desc"))
        {
            let container = unwrap_container(base).unwrap();
            let mut dict = unwrap_dict(container).unwrap();
            let gatt_char = dict
                .remove(&params::Base::String(
                    "org.bluez.GattCharacteristic1".into(),
                ))
                .expect("char object path should always have GattCharacteristic1");
            let gatt_char = unwrap_container(gatt_char).unwrap();
            let mut gatt_char = unwrap_dict(gatt_char).unwrap();
            let uuid = gatt_char
                .remove(&params::Base::String("UUID".into()))
                .expect("char object should always have a UUID");
            let uuid = unwrap_container(uuid).unwrap();
            let uuid = unwrap_variant(uuid).unwrap();
            let uuid = uuid.value;
            let uuid = unwrap_base(uuid).unwrap();
            let uuid = dbg!(unwrap_string(uuid).unwrap());

            if &uuid == char_uuid.as_ref() {
                return Ok(Some(path));
            }
        }
        Ok(None)
    }
}