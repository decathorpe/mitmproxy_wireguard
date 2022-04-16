import asyncio
import logging
import time
from typing import Optional

import mitmproxy_wireguard

LOG_FORMAT = "[%(asctime)s %(levelname)-5s %(name)s] %(message)s"
TIME_FORMAT = "%Y-%m-%dT%H:%M:%SZ"


class OnEvent:
    def __init__(self):
        self.connection: Optional[mitmproxy_wireguard.ServerConnection] = None

    def __call__(self, event):
        print(f"{event=}")
        if isinstance(event, mitmproxy_wireguard.ConnectionEstablished):
            print(f"{event.src_addr=}")
        elif isinstance(event, mitmproxy_wireguard.DataReceived):
            self.connection.send(event.connection_id, event.data)


async def main():
    logging.basicConfig(format=LOG_FORMAT, datefmt=TIME_FORMAT)
    logging.getLogger().setLevel(logging.DEBUG)
    logging.Formatter.converter = time.gmtime

    on_event = OnEvent()
    connection = mitmproxy_wireguard.start_server("0.0.0.0", 51820, on_event)
    on_event.connection = connection

    logging.info("Waiting 60 seconds for incoming messages ...")
    await asyncio.sleep(60)


if __name__ == "__main__":
    asyncio.run(main(), debug=True)
