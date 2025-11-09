# UDP API Definition

**Author:** Exp & Epoximator & Ommina  
**Version:** 0.03.730 (2015-03-25)  
**Version number used for protover parameter:** "3"

## IMPORTANT INFORMATION FOR ALL INTERESTED:

- If you are mainly interested in notifications and private messaging, check out our [Jabber](/Jabber) and [RSSRDF](/RSSRDF) support first.
- The UDP API is not an appropriate choice if you desire to download a local copy of the AniDB database.
- If you want to create a client you have to register it [here](https://anidb.net/perl-bin/animedb.pl?show=client) and [here](/UDP_Clients).
- Check out the clients that are being developed. There exists usable code in many different languages already.
- If you have suggestions for improvements or new features use the [development](/UDP_API_DEV) page.
- Please also take a look at the [API](/API) page.

## Formats used in this Spec

- `{vartype varname}` - Specifies what shall be inserted at this point and its type.
- `[...]` - Everything between [ and ] is optional.

### Used types

- **int2** - 2 byte Integer (in string representation)
- **int4** - 4 byte Integer (in string representation)
- **boolean** - true or false - use '1' for true, '0' for false
- **str** - String (UDP packet length restricts string size to 1400 bytes)
- **hexstr** - a hex representation of a decimal value, two characters per byte. If multiple bytes are represented, byte one is the first two characters of the string, byte two the next two, and so on.

## Content Encoding

- **Default character encoding:** ASCII
- **Escape scheme for option values (to server):** html form encoding + newline
  - This means you have to encode at least & in your option values in html form encoding style (&amp;) before sending them to the api server. All other html form encodings are optional.
  - All newlines should be replaced by `<br />`
- **Escape scheme for returned data fields (from server):** ', | and newline
  - Content newlines are encoded as `<br />`, ' is encoded as ` and | is encoded as /.
- **Dates** are returned in unix time (number of seconds elapsed since January 1, 1970 00:00:00 UTC)

## Basics

### General

The network communication is packet and line based. Each AniDB API command is exactly one UDP packet containing one line. Results are sent as one packet but may consist out of multiple lines. A return value always starts with a 3 byte result code followed by a human readable version of the result code. Be aware that important data fields may be returned directly after the return code (see: 200,201,209,271,272,504). The meaning for all result codes can be found in this document. If there is more than one entry returned, it's one entry per line. Lines are terminated by a \n (no dos linefeed \r). The elements of a format string are separated by a "|" character.

```
{three digit return code} {str return string}\n
{data field 0}|{data field 1}|...|{data field n}
```

#### IMPORTANT:

- All commands except PING, ENCRYPT, ENCODING, AUTH and VERSION requires login, meaning that the session tag must be set (s=xxxxx).
- A client should ignore any additional trailing data fields it doesn't expect. Additional fields are going to be added to the output of some commands from time to time. Those additional fields will simply be appended to the end of the existing output. A client should be written in a way so that it is not affected by such new fields.
- Due to the length constraints of an UDP package (over PPPoE) the replies from the server will never exceed 1400 bytes. String fields will be truncated if necessary to ensure this. No warnings are given when this happens.
- A client should handle all possible return codes for each command.
- **Possible return codes for all commands:**
  - 505 ILLEGAL INPUT OR ACCESS DENIED
  - 555 BANNED
    - {str reason}
  - 598 UNKNOWN COMMAND
  - 600 INTERNAL SERVER ERROR
  - 601 ANIDB OUT OF SERVICE - TRY AGAIN LATER
  - 602 SERVER BUSY - TRY AGAIN LATER
  - 604 TIMEOUT - DELAY AND RESUBMIT
- **Additional return codes for all commands that require login:**
  - 501 LOGIN FIRST
  - 502 ACCESS DENIED
  - 506 INVALID SESSION

> **Note:** The '555 BANNED' message should not be expected by the client. This message is only enabled temporary to help developers understand what they are doing wrong.

> **Note:** While in daily maintenance the AniDB API will reply with `601 ANIDB OUT OF SERVICE - TRY AGAIN LATER` to all commands. If a client receives such a return value it should wait at least 30 minutes before trying again.

### Server / UDP Connection

The Client has to send data packets to:

- **Server:** api.anidb.net
- **Port:** 9000/UDP

The servername and port should not be hardcoded into a frontend but should be read from a configuration file.

### Server Errors

- At any time the API might return a fatal error of the form:
  - 6xx ERROR DESCRIPTION
- Possible codes are 600-699.
- Occurrences of these errors (except 601 and 602) should be reported to Ommina.

> **Note:** 6XX messages do not always return the tags given with the command which caused the error!

### Connection Problems

There are many ways for a client to end up banned or the API might currently be handling too many concurrent connections. If a client does not get any reply to an AUTH command it should start to increase the retry delay exponentially with every failed login attempt. (i.e. try again after 30 seconds if the first login attempt failed, if the second fails too retry after 2 minutes, then 5 minutes, then 10 minutes, then 30 minutes, ... until you reach a retry delay of ~2-4h.)

### Local Port

A client should select a fixed local port >1024 at install time and reuse it for local UDP Sockets. If the API sees too many different UDP Ports from one IP within ~1 hour it will ban the IP. (So make sure you're reusing your UDP ports also for testing/debugging!)

The local port may be hardcoded, however, an option to manually specify another port should be offered.

**Note when behind a NAT/masquerading router:**

A session between the server and a client is identified by the IP and port used by the client. So when the port (or IP) changes within a session the client has to authenticate again. If a client is behind a NAT router it can't actually control the local port used for the connection. The router will normally translate the port to support several computers on a LAN to share the Internet connection. The public port (as determined by the router and seen by the server) which has been assigned to the connection will only be reserved for as long as it is in use. This means that the router will usually deallocate the port after a fixed timeout period (eg. five, ten or 15 minutes). Once that happens the client will no longer be able to receive UDP messages from the server (the messages will be discarded as undeliverable by the router) and a new port will be selected once the client tries to send a message to the server (which will result in a new connection session - Note: This could get you banned!, see above). So in order to keep a session over a NAT router alive, the client has to ping the server within the router's deallocation period to prevent a timeout.

The client can decide whether it is behind a NAT router or not by adding nat=1 to the AUTH command. This will cause the response to include the IP and port as seen by the server. If the port differs from the port reported by the local socket, the connection subject to NAT and the client should issue PING commands in regular intervals. Please do not send pings more often than is necessary to keep NAT connections alive.

### Flood Protection

To prevent high server load the UDP API server enforces a strict flood protection policy.

- **Short Term:**
  - A Client MUST NOT send more than 0.5 packets per second (that's one packet every two seconds, not two packets a second!)
  - The server will start to enforce the limit after the first 5 packets have been received.
- **Long Term:**
  - A Client MUST NOT send more than one packet every four seconds over an extended amount of time.
  - An extended amount of time is not defined. Use common sense.

Once a client exceeds a rate limit all further UDP packets from that client will be dropped without feedback until the client's packet rate is back down to acceptable levels.

> **Note:** Dropped packets are still taken into account for the packet rate. Meaning if you continuously send packets your client will be banned forever.

Generally clients should be written in a way to minimize server and network load. You should always keep that in mind.

Abusive clients may be banned completely.

> **Note:** If you suddenly stop getting replies from the AniDB API or you normally don't have a notable packetloss and from one point on you suddenly note a high packetloss percentage you have most likely entered a critical flood ratio. If the AniDB API isn't answering at all it might either be down or you have been banned, this is normally caused by violating the API specs (i.e. too many connections using different ports from one IP, too many auth failures, ...) (usually lasts 30 minutes). If you're experiencing packet loss you have probably reached the rate limit and the API starts to randomly drop incoming packets from your IP. (can be solved by enforcing a delay between multiple commands when you're sending a batch)

### Anti Leech Protection

The API should not be used to "download" AniDB. If such attempts are detected you will get banned.

### Caching

To minimize server and network load a client should use local caching in order to prevent repeated API requests for the same data.

A client should locally cache FILE/EP/ANIME/GROUP/... info wherever possible for extended amounts of time. (I.e. if a client is used to scan a local folder with anime files and add them via API to a users MyList then it shall only ask for all files in the first run and cache the info for all files known to AniDB. If run again over the same folder it shall only check those files which were unknown to AniDB at the time of the last check.)

Later versions of the API might enforce this by banning clients which ask for the same information more than once every week/month.

### Tag option

The API will add a user defined string at the beginning of each reply line separated with a space, if desired.

- To enable add the `tag={str usertag}` option to a command.
- A tag is only valid for the command it was send with, meaning it is not persistent. If you want to have tags in front of all reply lines you will have to append the tag option to each command you send to the server.
- Tags are meant to enable a client to handle more than one request/reply at a time.

**Usage example:**

```
AUTH user=baka&pass=baka&protover=25&client=someclient&clientver=1&tag=abc123
abc123 200 Jxqxb LOGIN ACCEPTED
```

or

```
LOGOUT s=Jxqxb&tag=byebye
byebye 203 LOGGED OUT
```

> **Note:** The tag option is valid for all api commands and is thus not explicitly listed in the description of each command.

### Data Indexes (fid,aid,eid,gid,lid)

- All indexes start at 1 (not 0).
- It is possible for table entries to have id fields with a value of 0 (i.e. gid). Those are to be interpreted as "NONE" or "NULL".
- An ID is never reused. That means after an entry is deleted no new entry will ever have that ID again.
- Mylist IDs (lid) are globally unique, not per-user unique.

## Authing Commands

> **Note:** ANY command which requires parameters may return: 505 ILLEGAL INPUT OR ACCESS DENIED

### AUTH: Authing to the AnimeDB

**Command String:**

```
AUTH user={str username}&pass={str password}&protover={int4 apiversion}&client={str clientname}&clientver={int4 clientversion}[&nat=1&comp=1&enc={str encoding}&mtu={int4 mtu value}&imgserver=1]
```

**Possible Replies:**

- 200 {str session_key} LOGIN ACCEPTED
- 201 {str session_key} LOGIN ACCEPTED - NEW VERSION AVAILABLE
- 500 LOGIN FAILED
- 503 CLIENT VERSION OUTDATED
- 504 CLIENT BANNED - {str reason}
- 505 ILLEGAL INPUT OR ACCESS DENIED
- 601 ANIDB OUT OF SERVICE - TRY AGAIN LATER

**when nat=1:**
- 200 {str session_key} {str ip}:{int2 port} LOGIN ACCEPTED
- 201 {str session_key} {str ip}:{int2 port} LOGIN ACCEPTED - NEW VERSION AVAILABLE

**when imgserver=1:**
- 200 {str session_key} LOGIN ACCEPTED
  - {str image server name}
- 201 {str session_key} LOGIN ACCEPTED - NEW VERSION AVAILABLE
  - {str image server name}

**Info:**

> **Note:** The password is the normal AniDB website password! The password listed in the profile as API Password in only used for the optional encryption feature.

- The session_key is a String containing only a-z,A-Z,0-9 chars of a length of 4-8 characters.
- It has to be stored by the client and needs to be sent as parameter with every command which requires the user to logged in.
- The session_key String is appended as parameter "s", i.e. "PUSH notify=1&msg=1&s={str session_key}".
- On a 500 LOGIN FAILED message the client should request the user to enter username and password again.
- In case of a 501 LOGIN FIRST message the client should silently resend an auth command and send the failed command again.
- A 502 ACCESS DENIED message should abort the current action on the client side and display a message to the user.
- A 503 CLIENT VERSION OUTDATED message states that the udp server has been updated and does not support your client any longer. (protover is too low). A 201 message refers to a new version of the client software.
- A 506 INVALID SESSION means that either the session key parameter "s" was not provided with a command that requires it or the session key is no longer valid. The client should reissue an AUTH command.

> **Note:** A frontend shall expect 501 and 502 messages to be returned on ANY command.

> **Note:** AniDB usernames may only contain characters (a-z,A-Z), numbers (0-9), underscores (_) and hyphens (-).

- The client should send the apiversion of the AnimeDB API version it supports as value of the protover parameter.
- The client MAY NOT send anything but the version of the API Specs the author used to write the client! (as it is stated at the top of this file @ "Version number used for protover parameter")
- The API will compare that value to it's own version of the API and if the version of the client is older the API will decide whether the changes were significant enough to deny the old client access to the DB.
- The clientname shall be a lower-case string containing only the chars a-z of 4-16 chars length which identifies the client. (i.e. mykickassclient)
- The clientversion shall be a number starting with 1, increased on every change in the client.
- clientname and clientversion might be used by the API to distinguish between different clients and client versions if that should ever become necessary.

> **Important:**
> - A Login and its assigned session_key is valid until the virtual UDP connection times out or until a LOGOUT command is issued.
> - The virtual UDP connection times out if no data was received from the client for 35 minutes.
> - A client should issue a UPTIME command once every 30 minutes to keep the connection alive should that be required.
> - If the client does not use any of the notification/push features of the API it should NOT keep the connection alive, furthermore it should explicitly terminate the connection by issuing a LOGOUT command once it finished it's work.
> - If it is very likely that another command will be issued shortly (within the next 20 minutes) a client SHOULD keep the current connection open, by not sending a LOGOUT command.

> **Important:**
> - The client shall notify the user if it received a 201 message at login.
> - This means a new version of the client is available, however the old version is still supported otherwise a client banned message would have been returned.
> - The user should get a pop-up message on 503 and 504 messages telling him to update his client software.
> - (Note: 504 means that this version of the client is banned, not the user!)

- The 'nat' option makes the client able to detect whether it is behind a nat router or not. When the client is behind a nat router it should keep the "connection" alive with the PING command.

> **Important:** Make sure your client handles ALL possible AUTH return codes before giving out any versions!

- When enc=x is defined the server will change the encoding used to x.
- If the encoding is supported it will change right away (including the response) and be reset on logout/timeout.
- If not supported then the argument will be silently ignored. Use ENCODING to test what works.
- Supported encodings are [these](http://java.sun.com/j2se/1.5.0/docs/guide/intl/encoding.doc.html).
- comp=1 means that the client supports compression (DEFLATE).
  - The server will compress (instead of truncating) the datagrams when needed if this option is enabled.
  - The first two bytes of compressed datagrams will always be set to zero. (So tags should never start with that.)
- Default MTU is 1400. Minimum allowed is 400, maximum 1400, due PPPoE.

### LOGOUT: Logout

**Command String:**

```
LOGOUT
```

**Possible Replies:**

- 203 LOGGED OUT
- 403 NOT LOGGED IN

**Info:**

- This command only works if you are already logged in.
- A logout should ALWAYS be issued if the client is currently logged in and is either exiting or not expecting to send/receive any AniDB API packets for the next >= 30 minutes.

### ENCRYPT: Start Encrypted Session

Will cause all future messages from the server, except the first (the reply to the ENCRYPT command itself), to be encrypted (128 bit AES). The client will also have to encrypt all future requests sent to the server. All non-encrypted messages will be discarded by the server. The encryption key is the MD5 hash of a special UDP API Key (defined in the users profile) concatenated with the salt string as given in the reply to the ENCRYPT message. A normal AUTH message is still necessary to authenticate and should follow the ENCRYPT command once the API has acknowledged the encryption.

**Command String:**

```
ENCRYPT user={str name}&type={int2 type}
```

**Possible Replies:**

- 209 {str salt} ENCRYPTION ENABLED
- 309 API PASSWORD NOT DEFINED
- 509 NO SUCH ENCRYPTION TYPE
- 394 NO SUCH USER

**Info:**

- user is the user name.
- type is the type of encryption; 1 => 128 bit AES (only one defined).
- API Key is the one defined in the [profile settings page](https://anidb.net/perl-bin/animedb.pl?show=profile).
- It is not possible to disable the encryption once enabled while staying logged in.
- A logout (the logout message needs to be correctly encrypted) or timeout will disable the encryption.
- In order to minimize server load, encryption should be disabled by default and should have to be enabled manually by the user in the configuration options.
- The encryption key is md5(api_key_of_user+salt).
- Padding of the message needs to be done according to the PKCS5Padding scheme.

## Notify Commands

### Introduction

Broadly speaking, notifications provide an indication to the client that some event has occurred within the AniDB database.

There are three types:

- New file [notification](/Notifications). (Only anime type supported.)
- New private message notification.
- New buddy event notification.

Note that, while a user can subscribe to multiple 'new file' events (see [notifications](/Notifications)), at present, the UDP API only supports notifications of new files by anime. New files by group, or new files by producer, are NOT presently supported. Just the same, keeping in mind that the API is designed to potentially support such notifications in the future will help in understanding why some of the commands are structured the way they are.

The word "notification" is also used a bit inconsistently in this document. An AniDB notification is originally a "new file [notification](/Notifications)". It might be more correct to use the term "event" for the original "happening" and then "notification" as the means to notify the user (client). New-file, new-message, buddy-* and going-down are all events that results in notifications. Only the first two type of events are persistent, though, meaning they exist and remain in the same state until some user action affects them.

### Getting Notifications

Clients that wish to receive notifications have two routes available to them. They are by no means mutually exclusive and selecting one does not imply a client is unable to use commands from another.

#### Method One: Polling

With this method, the client contacts the server at some interval (no more than once every 20 minutes) to see if there are new file notifications waiting. If there are, the client can the get further details of the files in question. This is analogous to checking an email server every half hour to see if new email has arrived.

Its principal advantage is that it is easy to design and code. Blocking sockets are sufficient as the client can expect the reply received to correspond with the command sent.

The disadvantage of this approach is that it introduces a delay and some uncertainty in receiving notifications. If, for example, the user clears the notification on the website before the client collects it, the client will not learn of the new file. Similarly, if the user does not dismiss notifications via the site, the client will have an increasing amount of stale data to work though. Finally, notifications cleared by the client also clears them from the website, so users will need to be made aware of what is going on.

**A polling HOWTO:**

1. Use NOTIFY (no more than once every 20 minutes) to get the number of pending notifications
2. IF there are new notifications pending, use NOTIFYLIST to get a list of notification types and associated IDs.
3. Use NOTIFYGET to receive a notification, supplying the ID provided by NOTIFYLIST
4. Use NOTIFYACK to acknowledge a notification, supplying the ID provided by NOTIFYLIST (if desired)

> **Note:** The ID supplied by NOTIFYLIST will be the ID of the affected notification type. Since, at present, only the anime type is supported, this value is always an aid. In the future, it may represent a group or producer ID. In these cases, the str type value will indicate what entity the ID represents.

#### Method Two: Server PUSH

With this method, the server takes the active role in advising the client that a new file has arrived. The client must register with the server to receive further advice information, and will be responsible for keeping the login session from timing out, and any NAT router ports open. The UDP packet is sent to the ip and port from which the AUTH command was received.

This method compensates for the disadvantages of the polling method, but is more difficult to code. Blocking sockets are no longer an option, nor can a client make any assumptions that an incoming packet will necessarily be a reply to the last command sent. The tag option may be helpful here.

**A PUSH HOWTO:**

1. PUSH to register your client session.
2. Listen for 720-799 NOTIFICATIONs (not 290).
3. Use PUSHACK to to acknowledge the notification using the nid supplied with NOTIFICATION
4. Use NOTIFYGET to receive a notification, suppling the relid provided by NOTIFICATION (NOT the packet ID)
5. Use NOTIFYACK to acknowledge a notification, suppling the relid provided by NOTIFICATION (if desired)
6. Use UPTIME (with an interval between 30 and 35 minutes) to keep login session valid

It is probably a good idea to use tags to separate NOTIFICATIONs from the other communication. NOTIFICATIONs will never have tags.

### PUSH: UDP Notification Registration

Register your client as an observer for AniDB notification events for the current user. If you are registered for one or more event types the AniDB server will send an UDP packet (format see below) on each change which affects the current user.

**Command String:**

```
PUSH notify={boolean on_new_file}&msg={boolean on_new_private_message}[&buddy={boolean on_buddy_event}]
```

**Possible Replies:**

- 270 NOTIFICATION ENABLED

OR (if both values are 0)

- 370 NOTIFICATION DISABLED

**Info:**

- A client which has registered to receive UDP notification packets must:
  - Issue a PUSHACK command for each notification received with notify_packet_id provided in the notification packet.
  - Use PING to keep the connection alive (< 30 min).
  - Use UPTIME to ensure that the session is OK (>= 30 min).
- Every notification generated is resent 3 times unless acknowledged. After that the client is logged out.

### NOTIFY: Notifications

Get number of pending notifications (and number of online buddies).

**Command String:**

```
NOTIFY [buddy=1]
```

**Possible Replies:**

- 290 NOTIFICATION
  - {int4 pending_file_notifications}|{int4 number_of_unread_messages}

**when buddy=1:**
- 290 NOTIFICATION
  - {int4 pending_file_notifications}|{int4 number_of_unread_messages}|{int4 number_of_online_buddies}

**Info:**

- If the client did send a NOTIFY within the last 35 minutes and it was confirmed by AniDB then receiving a 501 LOGIN FIRST message for the next NOTIFY command shows that AniDB logged the client out because it did not respond to a PUSH Notification packet.
- There is no command to retrieve missed PUSH Notifications.

> **Note:** This command MUST NOT be issued more than once every 20 minutes.

### NOTIFYLIST: List Notification/Message IDs

List id of all pending (not acknowledged) new private message and new file notifications. Buddy events cannot be acknowledged.

**Command String:**

```
NOTIFYLIST
```

**Possible Replies:**

- 291 NOTIFYLIST
  - {str type}|{int4 id}
  - {str type}|{int4 id}
  - ...

**Info:**

- type is:
  - M for message entries
  - N for notification entries
- id is the identifier for the notification/message as required by NOTIFYGET. For messages it is the actual message id, for notifications it is the id of the related type; anime, group or producer. Since only file notifications related to anime is implemented atm, it is the anime id (aid).
- NOTIFYLIST returns one line per entry, if no entries are available only the first line of the reply is returned.

> **Note:** This command MUST NOT be issued regularly but should only be triggered by either a push message received by the client or a reply to a NOTIFY command which tells you that there are messages/notifications waiting.

### NOTIFYGET: Get Notification/Message

Receive private message or file notification.

**Command String:**

```
NOTIFYGET type={str type}&id={int4 id}
```

**Possible Replies:**

**when type = M:**
- 292 NOTIFYGET
  - {int4 id}|{int4 from_user_id}|{str from_user_name}|{int4 date}|{int4 type}|{str title}|{str body}
- 392 NO SUCH ENTRY

**when type = N:**
- 293 NOTIFYGET
  - {int4 relid}|{int4 type}|{int2 count}|{int4 date}|{str relidname}|{str fids}
- 393 NO SUCH ENTRY

**Info:**

- type is:
  - M for message entries
  - N for notification entries
- id is the identifier for the notification/message as given by NOTIFYLIST (or relid from 271 NOTIFICATION and mid from 272 NOTIFICATION)
- for message entries (M):
  - date is the time of the event (in seconds since 1.1.1970)
  - type is the type of the message (0=normal msg, 1=annonymous, 2=system msg, 3=mod msg)
- for notification entries (N):
  - relid is the id of the related type (anime)
  - relname is the name of the related type (anime)
  - type is the notification type (0=all, 1=new, 2=group, 3=complete)
  - count is the number of events pending for this subscription
  - date is the time of the event (in seconds since 1.1.1970)
  - fids is a comma separated list with the affected file ids

### NOTIFYACK: Acknowledge Notification/Message

This command will mark a message read or clear a new file notification. Buddy events are not acknowledgeable.

**Command String:**

```
NOTIFYACK type={str type}&id={int4 id}
```

**Possible Replies:**

**when type = M:**
- 281 NOTIFYACK SUCCESSFUL
- 381 NO SUCH ENTRY

**when type = N:**
- 282 NOTIFYACK SUCCESSFUL
- 382 NO SUCH ENTRY

**Info:**

- type is:
  - M for message entries
  - N for notification entries

### Notification Packet Format

**New File Notify:**

```
720 {int4 notify_packet_id} NOTIFICATION - NEW FILE {int4 fidlist}|{int2 reltype}|{int2 priority}
```

- fidlist is a comma separated list of file ids
- reltype is: 1 = anime, 2 = group, 3 = producer
- priority is: 0 = low, 1 = medium, 2 = high
- new file notifications are created as a batch, so it is not unusual to get several new files for a particular anime at once. It is in this case that a comma separated list of fids will be returned

> **Note:** Group (and producer) related file notifications are not implemented yet.

**New Private Message Notify:**

```
794 {int4 notify_packet_id} NOTIFICATION - NEW MESSAGE {int2 type}|{int4 date}|{int4 sent_by_uid}|{str sent_by_name}|{str subject}|{str body}|{int mid}
```

- type is the type of the message (0=normal msg, 1=anonymous, 2=system msg, 3=mod msg)
- date is the time the message was sent (in seconds since 1.1.1970)
- sender uid/sender name are the user id and user name of the sender
- subject is the message subject
- body is message body (can be truncated)
- mid is message id and can be used with NOTIFYACK

**Buddy Event Notify:**

```
753 {int4 notify_packet_id} NOTIFICATION - BUDDY {int4 buddy uid}|{int2 event type}
```

- Possible event types:
  - 0 => LOGIN
  - 1 => LOGOUT
  - 2 => ACCEPTED
  - 3 => ADDED

**Going Down Event Notify:**

```
799 {int4 notify_packet_id} NOTIFICATION - SHUTTING DOWN {int4 time offline}|{int4 comment}
```

- Clients with any notification on will receive the SHUTTING DOWN message before the API goes offline.
- Time offline is the time in minutes the API will be down, 0 if indefinite (client can direct user to the AniDB site for status updates).
- The comment is a short explanation for the downtime.
- Only one datagram will be sent, and the server will not listen for replies.

### PUSHACK: UDP Notification Acknowledge

Used to acknowledge notification packets (720-799). A client must be prepared to issue this command before using PUSH.

**Command String:**

```
PUSHACK nid={int4 notify_packet_id}
```

**Possible Replies:**

- 701 PUSHACK CONFIRMED
- 702 NO SUCH PACKET PENDING

**Info:**

- See: PUSH

## Notification Commands

### NOTIFICATIONADD: Add Anime or Group to Notify List

**Command String:**

**by anime id:**
```
NOTIFICATIONADD aid={int4 aid}&type={int2 type}&priority={int2 priority}
```

**by group id:**
```
NOTIFICATIONADD gid={int4 gid}&type={int2 type}&priority={int2 priority}
```

**Possible Replies:**

- 246 NOTIFICATION ITEM ADDED
  - {int4 notification id}
- 248 NOTIFICATION ITEM UPDATED
  - {int4 notification id}
- 399 NO CHANGES

**Info:**

- Priority values are 0: low, 1: medium, 2: high
- Type values are 0: all, 1: new, 2: group, 3: complete

### NOTIFICATIONDEL: Remove Anime or Group from Notify List

**Command String:**

**by anime id:**
```
NOTIFICATIONDEL aid={int4 aid}
```

**by group id:**
```
NOTIFICATIONDEL gid={int4 gid}
```

**Possible Replies:**

- 247 NOTIFICATION ITEM DELETED
  - {int4 notification_table}|{int4 relid}
- 324 NO SUCH NOTIFICATION ITEM

**Info:**

- notification_table values are 1: anime, 2: group
- relid value matches the aid/gid supplied

## Buddy Commands

Group of commands used to administrate your buddylist.

### BUDDYADD: Add a user to Buddy List

**Command String:**

```
BUDDYADD uid={int4 buddy uid}
BUDDYADD uname={int4 buddy name}
```

**Possible Replies:**

- 394 NO SUCH USER
- 255 BUDDY ADDED
- 355 BUDDY ALREADY ADDED

### BUDDYDEL: Remove a user from Buddy List

**Command String:**

```
BUDDYDEL uid={int4 buddy uid}
```

**Possible Replies:**

- 356 NO SUCH BUDDY
- 256 BUDDY DELETED

### BUDDYACCEPT: Accept user as Buddy

**Command String:**

```
BUDDYACCEPT uid={int4 user uid}
```

**Possible Replies:**

- 356 NO SUCH BUDDY
- 257 BUDDY ACCEPTED
- 357 BUDDY ALREADY ACCEPTED

### BUDDYDENY: Deny user as Buddy

**Command String:**

```
BUDDYDENY uid={int4 buddy uid}
```

**Possible Replies:**

- 394 NO SUCH USER
- 258 BUDDY DENIED
- 358 BUDDY ALREADY DENIED

### BUDDYLIST: Retrieve Buddy List

**Command String:**

```
BUDDYLIST startat={int2 start at #}
```

**Possible Replies:**

- 253 {int2 start} {int2 end} {int2 total} BUDDY LIST
  - {int4 uid}|{str username}|{int2 state}
  - ...

**Info:**

- state is a 16bit bit field in integer notation (lowest bit first):
  - bit 1 = this user is in your buddylist
  - bit 2 = this user has accepted you
  - bit 3 = this user is waiting for your approval

### BUDDYSTATE: Retrieve Buddy States

**Command String:**

```
BUDDYSTATE startat={int2 start at #}
```

**Possible Replies:**

- 254 {int2 start} {int2 end} {int2 total} BUDDY STATE
  - {int4 uid}|{int2 onlinestate}
  - ...

**Info:**

- onlinestate is a 16bit bit field in integer notation (lowest bit first):
  - bit 1 = http online (user has issued a HTTP request within the last 10 minutes)
  - bit 2 = udp api online (user is currently connected to the udp api server)
  - example: 0=offline, 1=http online, 2=udp api online, 3=http&udp api online

## Data Commands

### ANIME: Retrieve Anime Data

**Command String:**

**by aid:**
```
ANIME aid={int4 id}[&amask={hexstr}]
```

**by name:**
```
ANIME aname={str anime name}[&amask={hexstr}]
```

**Possible Replies:**

**If no amask is provided:**
- 230 ANIME
  - {int4 aid}|{int4 eps}|{int4 ep count}|{int4 special cnt}|{int4 rating}|{int4 votes}|{int4 tmprating}|{int4 tmpvotes}|{int4 review rating average}|{int4 reviews}|{str year}|{str type}|{str romaji}|{str kanji}|{str english}|{str other}|{str short names}|{str synonyms}|{str category list}
- 330 NO SUCH ANIME

**Info:**

- Fields are returned in the same order they appear in the amask field list: byte 1, bit 7 first
- Synonyms and short names are separated with '
- Category fields are separated with ',' and ordered by weight (desc). However, be aware that categories are no longer used nor updated internally, and category responses are only returned to avoid breaking older clients. Use tags as a replacement.
- By name: must be perfect match of romaji/kanji/english/other/synonym/short name.
- 'Producer id list' and 'producer name list' match the data returned by the former producer bits (but using the revised creator ids)
- Date flags are used to indicated an unknown value (unknown month, day, year)
- dateflags values:
  - bit0 set == Startdate, Unknown Day
  - bit1 set == Startdate, Unknown Month, Day
  - bit2 set == Enddate, Unknown Day
  - bit3 set == Enddate, Unknown Month, Day
  - bit4 set == AirDate in the Past/Anime has ended
  - bit5 set == Startdate, Unknown Year
  - bit6 set == Enddate, Unknown Year
- Note: The character id list is the first data to be truncated if needed. And then: tag list, synonym list, short name list. This applies to the FILE command too.
- Selecting an 'unused' or 'reserved' bit will return an "illegal input" (505) response.

#### amask

**Byte 1:**

| Bit | Data Field |
|-----|------------|
| 7 | aid |
| 6 | unused |
| 5 | year |
| 4 | type |
| 3 | related aid list |
| 2 | related aid type |
| 1 | category list |
| 0 | reserved |

**Byte 2:**

| Bit | Data Field |
|-----|------------|
| 7 | romaji name |
| 6 | kanji name |
| 5 | english name |
| 4 | other name |
| 3 | short name list |
| 2 | synonym list |
| 1 | retired |
| 0 | retired |

**Byte 3:**

| Bit | Data Field |
|-----|------------|
| 7 | episodes |
| 6 | highest episode number |
| 5 | special ep count |
| 4 | air date |
| 3 | end date |
| 2 | url |
| 1 | picname |
| 0 | category id list |

**Byte 4:**

| Bit | Data Field |
|-----|------------|
| 7 | rating |
| 6 | vote count |
| 5 | temp rating |
| 4 | temp vote count |
| 3 | average review rating |
| 2 | review count |
| 1 | award list |
| 0 | is 18+ restricted |

**Byte 5:**

| Bit | Data Field |
|-----|------------|
| 7 | ann id |
| 6 | allcinema id |
| 5 | animenfo id |
| 4 | tag list |
| 3 | tag weight list |
| 2 | date record updated |
| 1 | character id list |
| 0 | creator id list |

**Byte 6:**

| Bit | Data Field |
|-----|------------|
| 7 | main creator id list |
| 6 | main creator name list |
| 5 | unused |
| 4 | unused |
| 3 | unused |
| 2 | unused |
| 1 | specials count |
| 0 | credits count |

**Byte 7:**

| Bit | Data Field |
|-----|------------|
| 7 | other count |
| 6 | trailer count |
| 5 | parody count |
| 4 | unused |
| 3 | unused |
| 2 | unused |
| 1 | unused |
| 0 | unused |

**Related aid type (Byte 1, Bit 2):**

| value | meaning |
|-------|---------|
| 1 | sequel |
| 2 | prequel |
| 11 | same setting |
| 12 | alternative setting |
| 32 | alternative version |
| 41 | music video |
| 42 | character |
| 51 | side story |
| 52 | parent story |
| 61 | summary |
| 62 | full story |
| 100 | other |

**Examples:** (note that the given amask gives the same result as the default request)

```
> ANIME aid=1&amask=b2f0e0fc000000&s=xxxxx
< 230 ANIME
1|1999-1999|TV Series|Space,Future,Plot Continuity,SciFi,Space Travel,Shipboard,Other Planet,Novel,Genetic Modification,Action,Romance,Military,Large Breasts,Gunfights,Adventure,Human Enhancement,Nudity|Seikai no Monshou|星界の紋章|Crest of the Stars||13|13|3|853|3225|756|110|875|11
```

### ANIMEDESC: Retrieve Anime Description

**Command String:**

**by aid:**
```
ANIMEDESC aid={int4 id}&part={int4 partno}
```

**Possible Replies:**

- 233 ANIMEDESC
  - {int4 current part}|{int4 max parts}|{str description}
- 330 NO SUCH ANIME
- 333 NO SUCH DESCRIPTION

**Info:**

- The maximum length of the anime description is roughly 5000 characters, but the maximum length of a UDP packet is 1400 bytes
- Multiple calls to ANIMEDESC may be necessary to retrieve the complete text, retrieving separate 1400 byte parts with each call
- part is zero-based
- Note: No support, at present, for retrieving descriptions by title. aid only

**Examples:** (html escaped code intended)

```
> ANIMEDESC aid=3169&part=0&s=xxxxx
< 233 ANIMEDESC
0|1|As summer break arrives for the students, Jun Sakurada is busily studying on his own in the library, making up for time lost <cut>
```

### CALENDAR: Get Upcoming Titles

Returns the anime ids of the 25 most recently aired / released anime, directly followed by the next 25 anime due to be aired / released, ordered by start date.

**Command String:**

```
CALENDAR
```

**Possible Replies:**

- 297 CALENDAR
  - {int aid}|{int startdate}|{int dateflags}
  - ..repeated n times
- 397 CALENDAR EMPTY

**Info:**

- Takes no parameters (other than the session string)
- Titles returned are filtered by the 'show adult' preference of the logged in user. That is, users who have elected to hide adult content will have it hidden here as well.
- Do not depend on the command always returning exactly 50 titles; there may not always be 25 future titles pending, depending on the user's 'adult' preferences, and the date in relation to the relation to the current anime season.
- Date flags are used to indicated an unknown value (unknown month, day, year)
- dateflags values:
  - bit0 set == Startdate, Unknown Day
  - bit1 set == Startdate, Unknown Month, Day
  - bit2 set == Enddate, Unknown Day
  - bit3 set == Enddate, Unknown Month, Day
  - bit4 set == AirDate in the Past/Anime has ended
  - bit5 set == Startdate, Unknown Year
  - bit6 set == Enddate, Unknown Year

### CHARACTER: Get Character Information

Returns character details associated with a given character ID, including associated anime ids, episode ids, and seiyuu.

**Command String:**

```
CHARACTER charid={int characterid}
```

**Possible Replies:**

- 235 CHARACTER
  - {int charid}|{str character name kanji}|{str character name transcription}|{str pic}|{anime blocks}|{int episode list}|{int last update date}|{int2 type}|{str gender}
  - An 'anime block' is {int anime id},{int appearance},{int creatorid},{boolean is_main_seiyuu} repeated as many times as necessary, separated by a single quote ( ' ).
- 335 NO SUCH CHARACTER

**Info:**

- If no seiyuu is associated with the character for a given aid, the 'creatorid' and 'is_main_seiyuu' fields will be empty, but the commas will remain.
- An empty episode list is "undefined": no episode values have been added.
- appearance values: 0='appears in', 1='cameo appearance in', 2='main character in', 3='secondary cast in'
- type can be one of: (Note: this is subject to changes so don't rely on this mapping)
  - 1 => 'Character'
  - 2 => 'Mecha'
  - 3 => 'Organisation'
  - 4 => 'Vessel'
- gender can be one of: (Note: this is subject to changes so don't rely on this mapping)
  - "M" => 'Male'
  - "F" => 'Female'
  - "I" => 'Intersexual'
  - "D" => 'Dimorphic'
  - "-" => 'none/does not apply'
  - "?" => 'unknown'

**Example:**

```
CHARACTER charid=488&s=DChan
235 CHARACTER
488|ニコ・ロビン|Nico Robin|14789.jpg|4097,2,1900,1'69,2,1901,0'6199,0,1900,1'5691,0,1900,1'2644,0,,'4851,0,1900,1||1236938094
```

### CREATOR: Get Creator Information

**Command String:**

```
CREATOR creatorid={int creatorid}
```

**Possible Replies:**

- 245 CREATOR
  - {int creatorid}|{str creator name kanji}|{str creator name transcription}|{int type}|{str pic_name}|{str url_english}|{str url_japanese}|{str wiki_url_english}|{str wiki_url_japanese}|{int last update date}
- 345 NO SUCH CREATOR

**Info:**

- ANIME AMASK byte6, bit5
- type values: 1='person', 2='company', 3='collaboration'

**Example:**

```
245 CREATOR
718|GAINAX|Gainax|2|10092.png||http://www.gainax.co.jp/|Gainax|Gainax|1237048093
```

### EPISODE: Retrieve Episode Data

**Command String:**

**by eid:**
```
EPISODE eid={int eid}
```

**by anime and episode number:**
```
EPISODE aname={str anime name}&epno={int4 episode number}
EPISODE aid={int anime id}&epno={int4 episode number}
```

**Possible Replies:**

- 240 EPISODE
  - {int eid}|{int aid}|{int4 length}|{int4 rating}|{int votes}|{str epno}|{str eng}|{str romaji}|{str kanji}|{int aired}|{int type}
- 340 NO SUCH EPISODE

**Info:**

- length is in minutes
- Returned 'epno' includes special character (only if special) and padding (only if normal).
- Special characters are S(special), C(credits), T(trailer), P(parody), O(other).
- The type is the raw episode type, used to indicate numerically what the special character will be
- 1: regular episode (no prefix), 2: special ("S"), 3: credit ("C"), 4: trailer ("T"), 5: parody ("P"), 6: other ("O")

**Examples:** (html escaped code intended)

```
> EPISODE eid=1&s=xxxxx
< 240 EPISODE
1|1|24|400|4|01|Invasion|shinryaku|??

> EPISODE aname=Seikai no Monshou&epno=2&s=xxxxx
< 240 EPISODE
2|1|24|750|2|02|Kin of the Stars|Hoshi-tachi no Kenzoku|??????|1295059229|1
```

### FILE: Retrieve File Data

**Command String:**

**by fid:**
```
FILE fid={int4 id}&fmask={hexstr fmask}&amask={hexstr amask}
```

**by size+ed2k hash:**
```
FILE size={int8 size}&ed2k={str ed2khash}&fmask={hexstr fmask}&amask={hexstr amask}
```

**by anime, group and epno:**
```
FILE aname={str anime name}&gname={str group name}&epno={int4 episode number}&fmask={hexstr fmask}&amask={hexstr amask}
FILE aname={str anime name}&gid={int4 group id}&epno={int4 episode number}&fmask={hexstr fmask}&amask={hexstr amask}
FILE aid={int4 anime id}&gname={str group name}&epno={int4 episode number}&fmask={hexstr fmask}&amask={hexstr amask}
FILE aid={int4 anime id}&gid={int4 group id}&epno={int4 episode number}&fmask={hexstr fmask}&amask={hexstr amask}
```

**Possible Replies:**

- 220 FILE
  - {int4 fid}|{int4 aid}|{int4 eid}|{int4 gid}|{int4 state}|{int8 size}|{str ed2k}|{str anidbfilename}
- 220 FILE
  - {int4 fid}|...(data list)
- 322 MULTIPLE FILES FOUND
  - {int4 fid 0}|{int4 fid 1}|...|{int4 fid n}
- 320 NO SUCH FILE

**Info:**

- fid, aid, eid, gid are the unique ids for the file, anime, ep, group entries at AniDB.
- You can use those to create links to the corresponding pages at AniDB.
- anidbfilename is the AniDB filename for the file.
- However this name does not contain all the extra information of the filenames on AniDB and might be composed slightly different.
- fmask and amask are hexidecimal strings where each bit corresponds to a data field related to the specified file (see below). The data list received is sorted and returned in the same order as the tables: top to bottom, left to right, with fmask preceding amask. There is no provision to retrieve all fields. Further, requesting a 'unusued' or 'reserved' bit will return an "illegal input" error.
- fid is always returned as the first value, regardless of what masks are provided.
- Only the first matching file is returned when aname, gname and epno is used.
- "audio codec" and "audio bitrate" will return multiple values if there are multiple audio streams present in the file. Values will be separated by a single quote.
- For byte 1, bit 2 (other episodes): episode IDs will be followed by an integer percentage that indicate the percentage of the related episode this file covers. Typically 100%, there are cases where it will be 50, 30, or other values. This value provides no indication where, within an episode, the file represents. That is, 50% could mean the file covers the first half of the episode, the second half, or some 50% range in between.

#### State:

| bit / int value | meaning |
|-----------------|---------|
| 1 / 1 | FILE_CRCOK: file matched official CRC (displayed with green background in AniDB) |
| 2 / 2 | FILE_CRCERR: file DID NOT match official CRC (displayed with red background in AniDB) |
| 3 / 4 | FILE_ISV2: file is version 2 |
| 4 / 8 | FILE_ISV3: file is version 3 |
| 5 / 16 | FILE_ISV4: file is version 4 |
| 6 / 32 | FILE_ISV5: file is version 5 |
| 7 / 64 | FILE_UNC: file is uncensored |
| 8 / 128 | FILE_CEN: file is censored |

**examples:**

```
state ==== 9 ==> FILE_CRCOK+FILE_ISV3 ==> file matched official CRC and is version 3
state ==== 0 ==> - ==> file was not crc checked and is version 1
state ==== 34 ==> FILE_CRCERR+FILE_ISV5 ==> file DID NOT match official CRC and is version 5
state ==== 1 ==> FILE_CRCOK ==> file matched official CRC and is version 1
state ==== 8 ==> FILE_ISV3 ==> file was not CRC checked and is version 3
```

#### fmask:

**Byte 1:**

| Bit | Data Field |
|-----|------------|
| 7 | unused |
| 6 | aid |
| 5 | eid |
| 4 | gid |
| 3 | mylist id |
| 2 | other episodes |
| 1 | IsDeprecated |
| 0 | state |

**Byte 2:**

| Bit | Data Field |
|-----|------------|
| 7 | size |
| 6 | ed2k |
| 5 | md5 |
| 4 | sha1 |
| 3 | crc32 |
| 2 | unused |
| 1 | video colour depth |
| 0 | quality |

**Byte 3:**

| Bit | Data Field |
|-----|------------|
| 7 | source |
| 6 | audio codec list |
| 5 | audio bitrate list |
| 4 | video codec |
| 3 | video bitrate |
| 2 | video resolution |
| 1 | file type (extension) |
| 0 | dub language |

**Byte 4:**

| Bit | Data Field |
|-----|------------|
| 7 | sub language |
| 6 | length in seconds |
| 5 | description |
| 4 | aired date |
| 3 | unused |
| 2 | unused |
| 1 | anidb file name |
| 0 | mylist state |

**Byte 5:**

| Bit | Data Field |
|-----|------------|
| 7 | mylist filestate |
| 6 | mylist viewed |
| 5 | mylist viewdate |
| 4 | mylist storage |
| 3 | mylist source |
| 2 | mylist other |
| 1 | unused |
| 0 | unused |

#### amask:

**Byte 1:**

| Bit | Data Field |
|-----|------------|
| 7 | anime total episodes |
| 6 | highest episode number |
| 5 | year |
| 4 | type |
| 3 | related aid list |
| 2 | related aid type |
| 1 | category list |
| 0 | reserved |

**Byte 2:**

| Bit | Data Field |
|-----|------------|
| 7 | romaji name |
| 6 | kanji name |
| 5 | english name |
| 4 | other name |
| 3 | short name list |
| 2 | synonym list |
| 1 | retired |
| 0 | retired |

**Byte 3:**

| Bit | Data Field |
|-----|------------|
| 7 | epno |
| 6 | ep name |
| 5 | ep romaji name |
| 4 | ep kanji name |
| 3 | episode rating |
| 2 | episode vote count |
| 1 | unused |
| 0 | unused |

**Byte 4:**

| Bit | Data Field |
|-----|------------|
| 7 | group name |
| 6 | group short name |
| 5 | unused |
| 4 | unused |
| 3 | unused |
| 2 | unused |
| 1 | unused |
| 0 | date aid record updated |

**Examples:** (html escaped code intended)

```
> FILE size=177747474&ed2k=70cd93fd3981cc80a8ea6a646ff805c9&fmask=7FF8FEF8&amask=C000F0C0&s=xxxxx
< 220 FILE
312498|4688|69260|4243|0||0|1|177747474|70cd93fd3981cc80a8ea6a646ff805c9|b2a7c7d591333e20495de3571b235c28|7af9b962c17ff729baeee67533e5219526cd5095|a200fe73|high|DTV|Vorbis (Ogg Vorbis)|104|H264/AVC|800|704x400|japanese|english'english'english|1560||1175472000|26|26|01|The Wings to the Sky|Sora he no Tsubasa|????|#nanoha-DamagedGoodz|Nanoha-DGz
```

### GROUP: Retrieve Group Data

**Command String:**

**by gid:**
```
GROUP gid={int gid}
```

**by name/shortname:**
```
GROUP gname={str group name}
```

**Possible Replies:**

- 250 GROUP
  - {int gid}|{int4 rating}|{int votes}|{int4 acount}|{int fcount}|{str name}|{str short}|{str irc channel}|{str irc server}|{str url}|{str picname}|{int4 foundeddate}|{int4 disbandeddate}|{int2 dateflags}|{int4 lastreleasedate}|{int4 lastactivitydate}|{list grouprelations}
- 350 NO SUCH GROUP

**Info:**

- Requires login
- gname is an exact match of either a group name or short name
- As either name and short names are unique if there is a result from GROUP request by name that will be the only match
- dateflags values:
  - bit0 set == Foundeddate, Unknown Day
  - bit1 set == Foundeddate, Unknown Month, Day
  - bit2 set == Disbandeddate, Unknown Day
  - bit3 set == Disbandeddate, Unknown Month, Day
  - bit5 set == Foundeddate, Unknown Year
  - bit6 set == Disbandeddate, Unknown Year
- releasedate and activitydate are distinct. releasedate is the date a file was actually released by the group, where activitydate is the date of a file being added to AniDB. As such, lastrelease may very well be much older than lastactivity.
- groupreleations is a list of apostrophe-separated pairs, where each pair consists of {int4 othergroupid},{int2 relationtype}
  - relationtype:
    - 1 => "Participant in"
    - 2 => "Parent of"
    - 4 => "Merged from"
    - 5 => "Now known as"
    - 6 => "Other"

**Example:**

```
> GROUP gid=7091&s=bunny
< 250 GROUP
7091|832|1445|43|566|Frostii|Frostii|#frostii|irc.rizon.net|http://frostii.com|15844.jpg|1228089600|0|1|1301875200|1304222640|7255,1'3097,4'748,4'8106,1'8159,2'8402,1'8696,1'9022,1
```

### GROUPSTATUS: Get Completed Episode

Returns a list of group names and ranges of episodes released by the group for a given anime.

**Command String:**

```
GROUPSTATUS aid={int animeid}[&state={int completion_state}]
```

**Possible Replies:**

- 225 GROUPSTATUS
  - {int group id}|{str group name}|{int completion state}|{int last episode number}|{int rating}|{int votes}|{str episode range}
  - {int group id}|{str group name}|{int completion state}|{int last episode number}|{int rating}|{int votes}|{str episode range}
  - ... (repeated)
- 325 NO SUCH GROUPS FOUND
- 330 NO SUCH ANIME

**Info:**

- The seven fields will be repeated as necessary, one for each group, separated by a new line character
- Groups will be filtered by the languages selected in the user's profile
- If state is not supplied, groups with a completion state of 'ongoing', 'finished', or 'complete' are returned
- Groups are returned in order of descending episode count
- If there are more groups to return than can be stored in a UDP packet, additional groups will be silently discarded
- state values:
  - ongoing
  - stalled
  - complete
  - dropped
  - finished
  - specials only

**Example:**

```
> GROUP GROUPSTATUS aid=8692&s=vLl1N
< 225 GROUPSTATUS
7407|Coalgirls|3|25|839|12|1-25
9863|Hadena Subs|3|25|374|1|1-25
11951|ChaosBlades|3|25|0|0|1-25
[truncated]
```

### UPDATED: Get List of Updated Anime IDs

Returns a list of AniDB anime ids of anime that have been updated in in a given time frame, ordered by descending age (oldest to newest change).

**Command String:**

```
UPDATED entity=1&[age={int4 id}|time={int4 date}]
```

**Possible Replies:**

- 243 UPDATED
  - {int entity}|{int total count}|{int last update date}|{list aid}
- 343 NO UPDATES

**Info:**

- entity is always 1
- Either age OR time can be specified, but not both
- age is specified in days.
  - eg: age=30 requests a list of aid values of anime that have changed in the past 30 days
- time is specified in unix time.
  - eg: time=1264982400 requests a list of aid values of anime that have changed since 2010-02-01
- A maximum of 200 values will be returned
- count specifies the total number of items found for the given time period. In short, if this value is great than 200, you have not retrieved all applicable ids.
- last update date will contain the AniDB update time of the last aid to appear on the list
- A given list value will appear only once. If there have been multiple changes to an entity, its age will reflect the most recent change.

**An anime will be considered updated if:**

- A change is made to the anime record itself
- An main or official anime title is added, edited, or deleted (not short or synonym)
- An episode for the anime is added, or deleted (but NOT edited!)
- An episode title is added, edited, or deleted
- An anime relation is added, or deleted

## MyList Commands

### MYLIST: Retrieve MyList Data

**Command String:**

**by lid:** (mylist id)
```
MYLIST lid={int4 lid}
```

**by fid:**
```
MYLIST fid={int4 fid}
```

**by size+ed2k hash:**
```
MYLIST size={int4 size}&ed2k={str ed2khash}
```

**by anime + group + epno:**
```
MYLIST aname={str anime name}[&gname={str group name}&epno={int4 episode number}]
MYLIST aname={str anime name}[&gid={int4 group id}&epno={int4 episode number}]
MYLIST aid={int4 anime id}[&gname={str group name}&epno={int4 episode number}]
MYLIST aid={int4 anime id}[&gid={int4 group id}&epno={int4 episode number}]
```

**Possible Replies:**

- 221 MYLIST
  - {int4 lid}|{int4 fid}|{int4 eid}|{int4 aid}|{int4 gid}|{int4 date}|{int2 state}|{int4 viewdate}|{str storage}|{str source}|{str other}|{int2 filestate}
- 312 MULTIPLE MYLIST ENTRIES
  - {str anime title}|{int episodes}|{str eps with state unknown}|{str eps with state on hhd}|{str eps with state on cd}|{str eps with state deleted}|{str watched eps}|{str group 1 short name}|{str eps for group 1}|...|{str group N short name}|{str eps for group N}
- 321 NO SUCH ENTRY

**Info:**

- The state field provides information about the location and sharing state of a file in MyList.
- If files are added after hashing, a client should specify the state as 1 (on HDD) (if the user doesn't explicitly select something else).
- eps is a list of episodes, e.g. "1-12,14-16,T1"

#### States:

```
0 - unknown - state is unknown or the user doesn't want to provide this information
1 - internal storage - the file is stored on hdd (but is not shared)
2 - external storage - the file is stored on cd/dvd/...
3 - deleted - the file has been deleted or is not available for other reasons (i.e. reencoded)
4 - remote storage - the file is stored on NAS/cloud/...
```

#### Filestates: (for normal files, i.e. not generic)

```
0 => normal/original
1 => corrupted version/invalid crc
2 => self edited
10 => self ripped
11 => on dvd
12 => on vhs
13 => on tv
14 => in theaters
15 => streamed
100 => other
```

**Example:**

```
> MYLIST aname=gits sac&s=xxxxx
< 322 MULTIPLE FILES FOUND
Koukaku Kidoutai STAND ALONE COMPLEX|26||1-26|1-26,S2-S27|||V-A|S2-S27|LMF|20-26|KAA|1-26|AonE|1-19|Anime-MX|1-3,9-20
```

### MYLISTADD: Add file to MyList

The command string for MYLISTADD is made of up two blocks: a 'fileinfo' block, which limits the command to a specific file, or an 'animeinfo'/'groupinfo'/'episodeinfo' block, which can be used to specify a range of files, including generics. Additionally, a number of optional parameters can be included with either set.

#### Adding a single file to the MyList with the 'fileinfo' block

```
MYLISTADD fid={int4 fid}
```

or

```
MYLISTADD size={int4 size}&ed2k={str ed2khash}
```

or

```
MYLISTADD lid={int4 lid}&edit=1
```
(valid for edit only)

#### Adding one or more files to the MyList with the 'animeinfo'/'groupinfo'/'episodeinfo' block

Here, you have a number of options, depending on what data you have available. In brief, 'animeinfo' represents a choice between anime id and anime name. 'groupinfo' represents a choice between group id, group name, and generic=1. 'episodeinfo' represents a range of one or more episodes.

```
MYLISTADD aid={int4 aid}&gid={int gid}&epno={int4 episode number}
```

or

```
MYLISTADD aid={int4 aid}&gname={str group_name}&epno={int4 episode number}
```

or

```
MYLISTADD aid={int4 aid}&generic=1&epno={int4 episode number}
```

or

```
MYLISTADD aname={str anime_name}&gid={int gid}&epno={int4 episode number}
```

or

```
MYLISTADD aname={str anime_name}&gname={str group_name}&epno={int4 episode number}
```

or

```
MYLISTADD aname={str anime_name}&generic=1&epno={int4 episode number}
```

Each command listed can have, in addition, a number of optional components to provide further MyList details. Append these as desired.

```
&state={int2 state}
&viewed={boolean viewed}
&viewdate={int4 viewdate}
&source={str source}
&storage={str storage}
&other={str other}
```

Finally, edit=1 can be included to edit a MyList entry instead of creating a new one. When editing, optional values that are not supplied retain their original value. That is, they are not replaced with default or empty values. Only values supplied are updated.

**Possible Replies:**

- 320 NO SUCH FILE
- 330 NO SUCH ANIME
- 350 NO SUCH GROUP

**when edit=0 and adding by fid, size/ed2k:**
- 210 MYLIST ENTRY ADDED
  - {int4 mylist id of new entry}

**when edit=0 and adding by aname, aid:**
- 210 MYLIST ENTRY ADDED
  - {int4 number of entries added}
- 310 FILE ALREADY IN MYLIST
  - {int4 lid}|{int4 fid}|{int4 eid}|{int4 aid}|{int4 gid}|{int4 date}|{int2 state}|{int4 viewdate}|{str storage}|{str source}|{str other}|{int2 filestate}
- 322 MULTIPLE FILES FOUND
  - {int4 fid 1}|{int4 fid 2}|...|{int4 fid n}

**when edit=1:**
- 311 MYLIST ENTRY EDITED
- 311 MYLIST ENTRY EDITED
  - {int4 number of entries edited}
- 411 NO SUCH MYLIST ENTRY

**Info:**

- All data except lid/fid/size+hash/generic is optional.
- Viewed should be 0 for unwatched files and 1 for watched files.
- If viewdate is not specified, the current time will be used. The field will be disregarded if viewed=0.
- Other is the only field which may contain newlines, but they have to be stored as `<br />`
- For state values refer to: MYLIST
- If you want to change an existing entry and already know its MyList id (lid) please use the lid-version of this command. It put less load on the server.
- epno=0 means all eps (default), negative numbers means upto. (-12 -> upto 12)
- group is optional only when edit=1, meaning you can't add every file for an anime
- If the file already exists (response code 310), the current record will be returned, in the same format as the MYLIST response.

### MYLISTDEL: Remove file from MyList

**Command String:**

**by lid:** (mylist id)
```
MYLISTDEL lid={int4 lid}
```

**by fid:**
```
MYLISTDEL fid={int4 fid}
```

**by size+ed2k hash:**
```
MYLISTDEL size={int4 size}&ed2k={str ed2k hash}
```

**by anime + group + epno:**
```
MYLISTDEL aname={str anime name}[&gname={str group name}&epno={int4 episode number}]
MYLISTDEL aname={str anime name}[&gid={int4 group id}&epno={int4 episode number}]
MYLISTDEL aid={int4 anime id}[&gname={str group name}&epno={int4 episode number}]
MYLISTDEL aid={int4 anime id}[&gid={int4 group id}&epno={int4 episode number}]
```

**Possible Replies:**

- 211 MYLIST ENTRY DELETED
  - {int4 number of entries}
- 411 NO SUCH MYLIST ENTRY

**Info:**

- group is optional
- command will delete all enties found

### MYLISTSTATS : Retrieve MyList stats

**Command String:**

```
MYLISTSTATS
```

**Possible Replies:**

- 222 MYLIST STATS
  - {animes}|{eps}|{files}|{size of files}|{added animes}|{added eps}|{added files}|{added groups}|{leech %}|{glory %}|{viewed % of db}|{mylist % of db}|{viewed % of mylist}|{number of viewed eps}|{votes}|{reviews}|{viewed length in minutes}

**Info:**

- All fields are int

### VOTE: Vote for specified anime/episode/group

**Command String:**

**by id:**
```
VOTE type={int2 type}&id={int4 id}[&value={int4 vote value}&epno={int4 episode number}]
VOTE type={int2 type}&id={int4 id}[&value={int4 vote value}&epno={str episode number}]
```

**by name:**
```
VOTE type={int2 type}&name={string name}[&value={int4 vote value}&epno={int4 episode number}]
VOTE type={int2 type}&name={string name}[&value={int4 vote value}&epno={str episode number}]
```

**Possible Replies:**

- 260 VOTED
  - {str entity name}|{vote value}|{vote type}|{entity id}
- 261 VOTE FOUND
  - {str entity name}|{vote value}|{vote type}|{entity id}
- 262 VOTE UPDATED
  - {str entity name}|{old vote value}|{vote type}|{entity id}
- 263 VOTE REVOKED
  - {str entity name}|{revoked vote value}|{vote type}|{entity id}
- 360 NO SUCH VOTE
  - {str entity name}|0|{vote type}|{entity id}
- 361 INVALID VOTE TYPE
- 362 INVALID VOTE VALUE
- 363 PERMVOTE NOT ALLOWED
  - {str aname}|{vote value}|{type}|{entity id}
- 364 ALREADY PERMVOTED
  - {str entity name}|{existing vote value}|{vote type}|{entity id}

**Example:**

```
VOTE type=1&id=5101&value=950
260 VOTED Clannad|950|1|5101

VOTE type=1&id=5101&value=950&epno=S2
260 VOTED Another World: Tomoyo Arc|950|1|91981

VOTE type=6&id=91981&value=950
260 VOTED Another World: Tomoyo Arc|950|1|91981
```

**Info:**

- Type: 1=anime, 2=anime tmpvote, 3=group, 6=episode
- Entity: anime, episode, or group
- For episode voting add epno on type=1, or specify type=6 if eid is known
- Value: negative number means revoke, 0 means retrieve (default), 100-1000 are valid vote values, rest is illegal
- Votes will be updated automatically (no questions asked)
- Tmpvoting when there exist a perm vote is not possible

### RANDOM: Get a random anime

**Command String:**

```
RANDOMANIME type={int4 type}
```

**Possible Replies:**

- 230 ANIME ... (see ANIME)

**Info:**

- type: 0=from db, 1=watched, 2=unwatched, 3=all mylist

## Misc Commands

### MYLISTEXPORT: Schedule a MyList Export

Queues a MyList Export by the AniDB Servers. As with a manual export request, exports are only done during periods when server load is low. As a result, exports may take up to 24 hours. The client submitting the request will receive an AniDB message when the export is ready to be collected.

Only one export can be in the queue at a time.

**Command String:**

```
MYLISTEXPORT [template={str template_name}|cancel=1]
```

**Possible Replies:**

- 217 EXPORT QUEUED
- 218 EXPORT CANCELLED
- 317 EXPORT NO SUCH TEMPLATE
- 318 EXPORT ALREADY IN QUEUE
- 319 EXPORT NO EXPORT QUEUED OR IS PROCESSING

**Info:**

- template_name must match an available export template on the MyList Export page.
- cancel will cancel any pending export request, queued either through UDP or the web server.
- Clients can subscribe the message notifications if they wish to be notified when an export is complete.

### PING: Ping Command

**Command String:**

```
PING [nat=1]
```

**Possible Replies:**

- 300 PONG
- {int4 port} (when nat=1)

**Info:**

- This command does not require a session.
- May be used by a frontend to determine if the API is still available.
- May be executed even if not yet logged in.
- The option nat=1 provides an easy way to determine if the router has changed the outgoing port.

### VERSION: Retrieve Server Version

**Command String:**

```
VERSION
```

**Possible Replies:**

- 998 VERSION
  - {str server version}

**Info:**

- This command does not require a session.

### UPTIME: Retrieve Server Uptime

**Command String:**

```
UPTIME
```

**Possible Replies:**

- 208 UPTIME
  - {int4 udpserver uptime in milliseconds}

**Info:**

- This command is the preferred way to check that the session is OK.

### ENCODING: Change Encoding for Session

Sets preferred encoding per session. The preferred way to do this is to use the enc argument for AUTH. This command is mostly for testing.

**Command String:**

```
ENCODING name={str encoding name}
```

**Possible Replies:**

- 219 ENCODING CHANGED
- 519 ENCODING NOT SUPPORTED

**Info:**

- This command does not require a session.
- Supported encodings: http://java.sun.com/j2se/1.5.0/docs/guide/intl/encoding.doc.html
- Default: ASCII.
- Resets to default on logout.

### SENDMSG: Send Message

**Command String:**

```
SENDMSG to={str username}&title={str title}&body={str body}
```

**Possible Replies:**

- 294 SENDMSG SUCCESSFUL
- 394 NO SUCH USER
- 501 LOGIN FIRST

**Note:**

- This command allows you to send an AniDB message.

> **Important:** title must not be longer than 50 chars. body must not be longer than 900 chars.

### USER: Retrieve User UID

**Command String:**

```
USER [user={str user name}|uid={int user id}]
```

**Possible Replies:**

- 394 NO SUCH USER
- 295 USER
  - {int4 uid}|{str username}

### STATS [disabled]

**Command String:**

```
STATS
```

**Possible Replies:**

- 206 STATS
  - {int4 animes)|{int4 eps}|{int4 files}|{int4 groups}|{int4 users}|{int8 total file size in bytes}|{int4 open creqs}

### TOP [disabled]

**Command String:**

```
TOP
```

**Possible Replies:**

- 207 TOP
  - {str longest mylist}|{int count}
  - {str largest mylist}|{int count}
  - {str most lame files}|{int count}
  - {str most indep. user}|{int count}
  - {str biggest leecher}|{int count}
  - {str most anime added}|{int count}
  - {str most eps added}|{int count}
  - {str most files added}|{int count}
  - {str most groups added}|{int count}
  - {str most votes}|{int count}
  - {str most reviews}|{int count}

**Info:**

- All strings are user names.
- 'Hide myself in IRC stats' applies for this too.

## Fatal Errors

At anypoint int the retrieval process you have to expect the following output:

- 6xx INTERNAL SERVER ERROR
  - ERROR: {str errormessage}

OR

- 6xx INTERNAL SERVER ERROR - {str errormessage}

Such errors should be reported back to Ommina.

(xx is a number between 00 and 99)

If you supply an unknown or unimplemented command you will get an error:

- 598 UNKNOWN COMMAND

## Return Codes

> **Note:** While the names below are the actual code strings returned by the server, clients are nonetheless encouraged to work with the numeric codes instead.

```
LOGIN_ACCEPTED = 200
LOGIN_ACCEPTED_NEW_VERSION = 201
LOGGED_OUT = 203
RESOURCE = 205
STATS = 206
TOP = 207
UPTIME = 208
ENCRYPTION_ENABLED = 209
MYLIST_ENTRY_ADDED = 210
MYLIST_ENTRY_DELETED = 211
ADDED_FILE = 214
ADDED_STREAM = 215
EXPORT_QUEUED = 217
EXPORT_CANCELLED = 218
ENCODING_CHANGED = 219
FILE = 220
MYLIST = 221
MYLIST_STATS = 222
WISHLIST = 223
NOTIFICATION = 224
GROUP_STATUS = 225
WISHLIST_ENTRY_ADDED = 226
WISHLIST_ENTRY_DELETED = 227
WISHLIST_ENTRY_UPDATED = 228
MULTIPLE_WISHLIST = 229
ANIME = 230
ANIME_BEST_MATCH = 231
RANDOM_ANIME = 232
ANIME_DESCRIPTION = 233
REVIEW = 234
CHARACTER = 235
SONG = 236
ANIMETAG = 237
CHARACTERTAG = 238
EPISODE = 240
UPDATED = 243
TITLE = 244
CREATOR = 245
NOTIFICATION_ENTRY_ADDED = 246
NOTIFICATION_ENTRY_DELETED = 247
NOTIFICATION_ENTRY_UPDATE = 248
MULTIPLE_NOTIFICATION = 249
GROUP = 250
CATEGORY = 251
BUDDY_LIST = 253
BUDDY_STATE = 254
BUDDY_ADDED = 255
BUDDY_DELETED = 256
BUDDY_ACCEPTED = 257
BUDDY_DENIED = 258
VOTED = 260
VOTE_FOUND = 261
VOTE_UPDATED = 262
VOTE_REVOKED = 263
HOT_ANIME = 265
RANDOM_RECOMMENDATION = 266
RANDOM_SIMILAR = 267
NOTIFICATION_ENABLED = 270
NOTIFYACK_SUCCESSFUL_MESSAGE = 281
NOTIFYACK_SUCCESSFUL_NOTIFICATION = 282
NOTIFICATION_STATE = 290
NOTIFYLIST = 291
NOTIFYGET_MESSAGE = 292
NOTIFYGET_NOTIFY = 293
SENDMESSAGE_SUCCESSFUL = 294
USER_ID = 295
CALENDAR = 297
PONG = 300
AUTHPONG = 301
NO_SUCH_RESOURCE = 305
API_PASSWORD_NOT_DEFINED = 309
FILE_ALREADY_IN_MYLIST = 310
MYLIST_ENTRY_EDITED = 311
MULTIPLE_MYLIST_ENTRIES = 312
WATCHED = 313
SIZE_HASH_EXISTS = 314
INVALID_DATA = 315
STREAMNOID_USED = 316
EXPORT_NO_SUCH_TEMPLATE = 317
EXPORT_ALREADY_IN_QUEUE = 318
EXPORT_NO_EXPORT_QUEUED_OR_IS_PROCESSING = 319
NO_SUCH_FILE = 320
NO_SUCH_ENTRY = 321
MULTIPLE_FILES_FOUND = 322
NO_SUCH_WISHLIST = 323
NO_SUCH_NOTIFICATION = 324
NO_GROUPS_FOUND = 325
NO_SUCH_ANIME = 330
NO_SUCH_DESCRIPTION = 333
NO_SUCH_REVIEW = 334
NO_SUCH_CHARACTER = 335
NO_SUCH_SONG = 336
NO_SUCH_ANIMETAG = 337
NO_SUCH_CHARACTERTAG = 338
NO_SUCH_EPISODE = 340
NO_SUCH_UPDATES = 343
NO_SUCH_TITLES = 344
NO_SUCH_CREATOR = 345
NO_SUCH_GROUP = 350
NO_SUCH_CATEGORY = 351
BUDDY_ALREADY_ADDED = 355
NO_SUCH_BUDDY = 356
BUDDY_ALREADY_ACCEPTED = 357
BUDDY_ALREADY_DENIED = 358
NO_SUCH_VOTE = 360
INVALID_VOTE_TYPE = 361
INVALID_VOTE_VALUE = 362
PERMVOTE_NOT_ALLOWED = 363
ALREADY_PERMVOTED = 364
HOT_ANIME_EMPTY = 365
RANDOM_RECOMMENDATION_EMPTY = 366
RANDOM_SIMILAR_EMPTY = 367
NOTIFICATION_DISABLED = 370
NO_SUCH_ENTRY_MESSAGE = 381
NO_SUCH_ENTRY_NOTIFICATION = 382
NO_SUCH_MESSAGE = 392
NO_SUCH_NOTIFY = 393
NO_SUCH_USER = 394
CALENDAR_EMPTY = 397
NO_CHANGES = 399
NOT_LOGGED_IN = 403
NO_SUCH_MYLIST_FILE = 410
NO_SUCH_MYLIST_ENTRY = 411
MYLIST_UNAVAILABLE = 412
LOGIN_FAILED = 500
LOGIN_FIRST = 501
ACCESS_DENIED = 502
CLIENT_VERSION_OUTDATED = 503
CLIENT_BANNED = 504
ILLEGAL_INPUT_OR_ACCESS_DENIED = 505
INVALID_SESSION = 506
NO_SUCH_ENCRYPTION_TYPE = 509
ENCODING_NOT_SUPPORTED = 519
BANNED = 555
UNKNOWN_COMMAND = 598
INTERNAL_SERVER_ERROR = 600
ANIDB_OUT_OF_SERVICE = 601
SERVER_BUSY = 602
NO_DATA = 603
TIMEOUT - DELAY AND RESUBMIT = 604
API_VIOLATION = 666
PUSHACK_CONFIRMED = 701
NO_SUCH_PACKET_PENDING = 702
VERSION = 998
```
