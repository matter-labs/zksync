CREATE TYPE token_kind AS ENUM ('ERC20', 'NFT', 'None');
ALTER TABLE tokens ADD COLUMN kind token_kind NOT NULL DEFAULT 'ERC20';
UPDATE tokens SET kind = 'NFT' WHERE is_nft = true;
ALTER TABLE tokens DROP COLUMN is_nft;
