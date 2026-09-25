-- Xbox device tokens were only used by the SISU sign-in flow, which was
-- replaced by the standard Microsoft OAuth -> Xbox Live -> XSTS flow.
DROP TABLE IF EXISTS minecraft_device_tokens;
