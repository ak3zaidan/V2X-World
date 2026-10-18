# Practical Dynamic Group Signature with Efficient Concurrent Joins and Batch Verifications
Hyoseung Kim, Youngkyung Lee, Michel Abdalla, J. H. Park · IACR Cryptology ePrint Archive 2021 · https://doi.org/10.1016/j.jisa.2021.103003 · relevance: medium · basis: abstract

**Problem:** Dynamic group signatures ( DGS ) enable a user to generate a signature on behalf of a group of users, allowing a prospective user to join via an appropriate join protocol. A natural security requirement in the dynamic setting is to permit an adversary to concurrently perform join protocol executions.
**Approach:** In this paper, we propose a new practical DGS scheme that supports not only efﬁcient concurrent joins but also batch veriﬁcations.
**Tools and scale:** The concurrent security is proven by showing that our join protocols are simulated without any knowledge extractor in security analysis.
**Results:** Also, DGS schemes have to provide efﬁcient batch veriﬁcations for practical applications such as Vehicle-to-Vehicle (V2V) and Vehicle-to-Infrastructure (V2I) communication, where a large number of group signatures should be veriﬁed in a very short time. In terms of efﬁciency, when considering a type-3 pairing, our DGS scheme has the advantages that the signature generation and veriﬁcation are faster and especially our batch veriﬁcation is at least 7 times faster in case of verifying 100 signatures, compared to other comparable ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Bespoke VANET/IoV authentication or privacy scheme (CPPA/certificateless/batch/blockchain style), evaluated by analysis or small simulation; not IEEE 1609.2/SCMS and no closed-loop safety evaluation.
**Novelty implications:** Adds to the large body of standalone authentication schemes; does not touch end-to-end SCMS simulation or safety impact of security.
