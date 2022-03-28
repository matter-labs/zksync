ALTER TABLE tokens ADD COLUMN is_nft boolean NOT NULL DEFAULT false;
UPDATE tokens SET is_nft = true WHERE kind = 'NFT';
ALTER TABLE tokens DROP COLUMN kind;
DROP TYPE token_kind;
