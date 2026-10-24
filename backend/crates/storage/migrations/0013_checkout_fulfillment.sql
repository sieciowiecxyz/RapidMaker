-- Historical attempts may have ambiguous equal-price carrier offers: never guess.
ALTER TABLE payment_attempts ADD COLUMN offer_id text CHECK(octet_length(offer_id) BETWEEN 1 AND 128);
ALTER TABLE orders ADD COLUMN fulfillment jsonb CHECK(jsonb_typeof(fulfillment)='object' AND octet_length(fulfillment::text)<=131072);
WITH candidates AS MATERIALIZED (
 SELECT p.id, (array_agg(offer))[1] AS offer, d.input, d.quote
 FROM payment_attempts p JOIN checkout_drafts d ON d.id=p.draft_id
 CROSS JOIN LATERAL jsonb_array_elements(CASE WHEN jsonb_typeof(d.quote->'offers')='array' THEN d.quote->'offers' ELSE '[]'::jsonb END) AS offer
 WHERE d.locked AND offer->>'total_grosze'=p.amount_grosze::text
 GROUP BY p.id,d.input,d.quote HAVING count(*)=1
), recorded AS (
 UPDATE payment_attempts p SET offer_id=c.offer->>'id' FROM candidates c
 WHERE p.id=c.id AND jsonb_typeof(c.offer->'id')='string' AND octet_length(c.offer->>'id') BETWEEN 1 AND 128
 RETURNING p.id
)
UPDATE orders o SET fulfillment=jsonb_build_object('delivery',c.input->'delivery','offer',c.offer,'packing',c.quote->'packing','point',c.quote->'point','packaging_grosze',c.quote->'packaging_grosze')
FROM candidates c JOIN recorded r ON r.id=c.id WHERE o.payment_attempt_id=c.id;
