import asyncio

import mitmproxy_wireguard


# simple echo server
def on_event(event):
    print(f"{event=}")
    if isinstance(event, mitmproxy_wireguard.ConnectionEstablished):
        print(f"{event.src_addr=}")
    elif isinstance(event, mitmproxy_wireguard.DataReceived):
        pass  # server.tcp_send(event.connection_id, event.data)


async def main():
    print("Listening on 0.0.0.0:51820 ...")
    server = await mitmproxy_wireguard.start_server("0.0.0.0", 51820, on_event)

    print(f"{server=}")

    print("Waiting 60 seconds for incoming messages ...")
    await asyncio.sleep(60)


if __name__ == "__main__":
    print(f"{dir(mitmproxy_wireguard)=}")

    asyncio.run(main(), debug=True)
