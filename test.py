import asyncio

import mitmproxy_wireguard

print(f"{dir(mitmproxy_wireguard)=}")


async def main():

    print(f"{mitmproxy_wireguard.keypair()=}")

    async def handle_connection(r: asyncio.StreamReader, w: asyncio.StreamWriter):
        print(f"connection task {w=}")
        print(f"{w.get_extra_info('peername')=}")
        for _ in range(2):
            print("reading...")
            try:
                data = await r.read(4096)
            except Exception as exc:
                print(f"read {exc=}")
                data = b""
            print(f"read complete. writing... {len(data)=} {data[:10]=} ")
            try:
                w.write(data.upper())
            except Exception as exc:
                print(f"write {exc=}")
            print("write complete. draining...")
            try:
                await w.drain()
            except Exception as exc:
                print(f"drain {exc=}")
            print("drained.")
        print("closing...")
        try:
            w.close()
        except Exception as exc:
            print(f"close {exc=}")
        print("closed.")

    def receive_datagram(data, src_addr, dst_addr):
        print(f"Echoing datagram... {data=} {src_addr=} {dst_addr=}")
        server.send_datagram(data.upper(), dst_addr, src_addr)
        print("done.")

    print("main")
    server = await mitmproxy_wireguard.start_server(
        "0.0.0.0",
        51820,
        "c72d788fd0916b1185177fd7fa392451192773c889d17ac739571a63482c18bb",
        ["DbwqnNYZWk5e19uuSR6WomO7VPaVbk/uKhmyFEnXdH8="],
        handle_connection,
        receive_datagram
    )
    print(f"{server=}")

    await asyncio.sleep(20)
    print("stopping")
    server.stop()
    # del server
    # no more messages
    await asyncio.sleep(3000)


if __name__ == "__main__":
    asyncio.run(main(), debug=True)
