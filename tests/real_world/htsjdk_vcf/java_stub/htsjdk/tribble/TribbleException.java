package htsjdk.tribble;

/** Minimal dependency boundary for exercising the untouched VCF sources. */
public class TribbleException extends RuntimeException {
    public TribbleException(final String message) {
        super(message);
    }

    public static class InvalidHeader extends TribbleException {
        public InvalidHeader(final String message) {
            super(message);
        }
    }

    public static class InternalCodecException extends TribbleException {
        public InternalCodecException(final String message) {
            super(message);
        }
    }
}
