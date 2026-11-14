//! Album art: read the GSMTC thumbnail and re-encode it as a small JPEG data URL.

use windows::core::Interface;
use windows::Graphics::Imaging::{
    BitmapAlphaMode, BitmapDecoder, BitmapEncoder, BitmapInterpolationMode, BitmapPixelFormat,
    SoftwareBitmap,
};
use windows::Storage::Streams::{
    DataReader, IInputStream, IRandomAccessStream, IRandomAccessStreamReference,
    InMemoryRandomAccessStream,
};

use super::model::{base64, fit_within};

/// Covers are shown at ≤ 140 logical px; 320 px keeps them sharp at 200% scale.
const MAX_SIDE: u32 = 320;
/// Hard cap from PLAN.md: a data URL must stay ≤ 300 KB.
const MAX_BYTES: usize = 300 * 1024;

/// Loads the thumbnail as a JPEG data URL. `Ok(None)` when it is too large to send.
pub fn load(reference: &IRandomAccessStreamReference) -> windows::core::Result<Option<String>> {
    let source: IRandomAccessStream = reference.OpenReadAsync()?.join()?.cast()?;
    let jpeg = reencode(&source)?;
    if jpeg.len() > MAX_BYTES {
        log::warn!("cover is {} bytes after re-encoding, skipped", jpeg.len());
        return Ok(None);
    }
    Ok(Some(format!("data:image/jpeg;base64,{}", base64(&jpeg))))
}

fn reencode(source: &IRandomAccessStream) -> windows::core::Result<Vec<u8>> {
    let decoder = BitmapDecoder::CreateAsync(source)?.join()?;
    let (width, height) = fit_within(decoder.PixelWidth()?, decoder.PixelHeight()?, MAX_SIDE);
    let bitmap = SoftwareBitmap::ConvertWithAlpha(
        &decoder.GetSoftwareBitmapAsync()?.join()?,
        BitmapPixelFormat::Bgra8,
        BitmapAlphaMode::Ignore,
    )?;

    let output = InMemoryRandomAccessStream::new()?;
    let encoder = BitmapEncoder::CreateAsync(BitmapEncoder::JpegEncoderId()?, &output)?.join()?;
    encoder.SetSoftwareBitmap(&bitmap)?;
    let transform = encoder.BitmapTransform()?;
    transform.SetScaledWidth(width)?;
    transform.SetScaledHeight(height)?;
    transform.SetInterpolationMode(BitmapInterpolationMode::Fant)?;
    encoder.FlushAsync()?.join()?;

    read_all(&output.GetInputStreamAt(0)?, output.Size()?)
}

fn read_all(stream: &IInputStream, size: u64) -> windows::core::Result<Vec<u8>> {
    let size = u32::try_from(size).map_err(|_| windows::core::Error::from_hresult(E_BOUNDS))?;
    let reader = DataReader::CreateDataReader(stream)?;
    reader.LoadAsync(size)?.join()?;
    let mut bytes = vec![0; size as usize];
    reader.ReadBytes(&mut bytes)?;
    Ok(bytes)
}

const E_BOUNDS: windows::core::HRESULT = windows::core::HRESULT(0x8000_000B_u32 as i32);
