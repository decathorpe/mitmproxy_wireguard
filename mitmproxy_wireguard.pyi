import asyncio
from collections.abc import Awaitable, Callable


class WireguardServer:
    def send_datagram(self, data: bytes, src_addr: tuple[str, int], dst_addr: tuple[str, int]) -> None:
        ...

    def stop(self) -> None:
        ...


async def start_server(
    host: str,
    port: int,
    private_key: str,
    peers: list[str],
    handle_connection: Callable[[asyncio.StreamReader, asyncio.StreamWriter], Awaitable[None]],
    receive_datagram: Callable[[bytes, tuple[str, int], tuple[str, int]], None],
) -> WireguardServer:
    ...

def keypair() -> tuple[str,str]:
    ...
