class PrimitiveGetterDispatchCoercion {
    int readLength(SamRecord record) {
        int length = record.getReadLength();
        return length;
    }
}

