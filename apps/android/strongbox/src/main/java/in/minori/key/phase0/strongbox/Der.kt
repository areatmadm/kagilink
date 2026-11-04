package `in`.minori.key.phase0.strongbox

/** Minimal definite-length DER reader, just enough for the key attestation extension header. */
internal class Der(private val b: ByteArray, private var pos: Int = 0, private val end: Int = b.size) {
    class Tlv(val tag: Int, val start: Int, val end: Int)

    fun next(): Tlv {
        require(pos + 2 <= end) { "DER truncated" }
        val tag = b[pos++].toInt() and 0xff
        require(tag and 0x1f != 0x1f) { "DER high tag number unsupported" }
        var len = b[pos++].toInt() and 0xff
        if (len >= 0x80) {
            val n = len and 0x7f
            require(n in 1..3 && pos + n <= end) { "DER bad length" }
            len = 0
            repeat(n) { len = (len shl 8) or (b[pos++].toInt() and 0xff) }
        }
        val start = pos
        require(len >= 0 && start + len <= end) { "DER length exceeds buffer" }
        pos = start + len
        return Tlv(tag, start, start + len)
    }

    fun intOf(t: Tlv, tag: Int): Int {
        require(t.tag == tag) { "DER tag ${t.tag} != $tag" }
        require(t.end - t.start in 1..4) { "DER integer size" }
        var v = 0
        for (i in t.start until t.end) v = (v shl 8) or (b[i].toInt() and 0xff)
        return v
    }

    fun bytesOf(t: Tlv, tag: Int): ByteArray {
        require(t.tag == tag) { "DER tag ${t.tag} != $tag" }
        return b.copyOfRange(t.start, t.end)
    }

    fun child(t: Tlv, tag: Int): Der {
        require(t.tag == tag) { "DER tag ${t.tag} != $tag" }
        return Der(b, t.start, t.end)
    }
}

/** The leading fields of the Android KeyDescription (OID 1.3.6.1.4.1.11129.2.1.17). */
internal class KeyDescription(
    val attestationVersion: Int,
    val attestationSecurityLevel: Int,
    val keymasterVersion: Int,
    val keymasterSecurityLevel: Int,
    val attestationChallenge: ByteArray,
) {
    companion object {
        /** [extensionValue] is X509Certificate.getExtensionValue(): a DER OCTET STRING wrapping the extension. */
        fun parse(extensionValue: ByteArray): KeyDescription {
            val outer = Der(extensionValue)
            val octet = outer.next()
            require(octet.tag == 0x04) { "extension not an OCTET STRING" }
            val seq = outer.child(octet, 0x04).next()
            val d = outer.child(octet, 0x04).child(seq, 0x30)
            return KeyDescription(
                attestationVersion = d.intOf(d.next(), 0x02),
                attestationSecurityLevel = d.intOf(d.next(), 0x0a),
                keymasterVersion = d.intOf(d.next(), 0x02),
                keymasterSecurityLevel = d.intOf(d.next(), 0x0a),
                attestationChallenge = d.bytesOf(d.next(), 0x04),
            )
        }
    }
}
