/**
 * Generates an filled data array.
 */
export function generateArray(length: number): Uint8Array {
    const data = new Uint8Array(length);
    for (let i = 0; i < length; i++) {
        data[i] = i % 255;
    }

    return data;
}
