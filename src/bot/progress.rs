//! Lecture d'un fichier qui signale sa progression par paliers de 10 % (comme `ProgressFile` du bot Python).

use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, ReadBuf};
use tokio::sync::mpsc::UnboundedSender;

/// Enveloppe un lecteur et envoie 10, 20 … 100 sur `tx` au fil des octets lus.
pub struct ProgressRead<R> {
    inner: R,
    total: u64,
    read: u64,
    step: u8,
    tx: UnboundedSender<u8>,
}

impl<R> ProgressRead<R> {
    pub fn new(inner: R, total: u64, tx: UnboundedSender<u8>) -> Self {
        Self {
            inner,
            total,
            read: 0,
            step: 0,
            tx,
        }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for ProgressRead<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let r = Pin::new(&mut self.inner).poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = r {
            self.read += (buf.filled().len() - before) as u64;
            if let Some(p) = (self.read * 100).checked_div(self.total) {
                let pct = p.min(100) as u8 / 10 * 10;
                if pct > self.step {
                    self.step = pct;
                    let _ = self.tx.send(pct);
                }
            }
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn signale_des_paliers_de_10_pour_cent() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let data = vec![1u8; 1000];
        let mut r = ProgressRead::new(std::io::Cursor::new(data), 1000, tx);
        let mut out = Vec::new();
        r.read_to_end(&mut out).await.unwrap();
        drop(r);
        let mut got = Vec::new();
        while let Some(p) = rx.recv().await {
            got.push(p);
        }
        assert_eq!(out.len(), 1000);
        assert_eq!(got.last(), Some(&100));
        assert!(got.windows(2).all(|w| w[0] < w[1] && w[1] % 10 == 0));
    }
}
