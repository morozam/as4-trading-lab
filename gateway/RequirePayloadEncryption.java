package lab.holodeck;

import java.util.HashSet;
import java.util.Set;
import org.apache.logging.log4j.Logger;
import org.holodeckb2b.common.errors.PolicyNoncompliance;
import org.holodeckb2b.common.handlers.AbstractUserMessageHandler;
import org.holodeckb2b.core.HolodeckB2BCore;
import org.holodeckb2b.core.pmode.PModeUtils;
import org.holodeckb2b.interfaces.core.IMessageProcessingContext;
import org.holodeckb2b.interfaces.pmode.IPMode;
import org.holodeckb2b.interfaces.pmode.ITradingPartnerConfiguration;
import org.holodeckb2b.interfaces.processingmodel.ProcessingState;
import org.holodeckb2b.interfaces.security.IEncryptionProcessingResult;
import org.holodeckb2b.interfaces.storage.IPayloadEntity;
import org.holodeckb2b.interfaces.storage.IUserMessageEntity;

/** Require encrypted payloads before business delivery; receipts are signal messages. */
public final class RequirePayloadEncryption extends AbstractUserMessageHandler {
    private static final String AES = "http://www.w3.org/2009/xmlenc11#aes128-gcm";

    @Override
    protected InvocationResponse doProcessing(IUserMessageEntity message,
            IMessageProcessingContext context, Logger log) throws Exception {
        IPMode mode = HolodeckB2BCore.getPModeSet().get(message.getPModeId());
        ITradingPartnerConfiguration receiver = PModeUtils.isHolodeckB2BInitiator(mode)
                ? mode.getInitiator() : mode.getResponder();
        if (receiver == null || receiver.getSecurityConfiguration() == null
                || receiver.getSecurityConfiguration().getEncryptionConfiguration() == null) {
            return InvocationResponse.CONTINUE;
        }
        Set<String> encrypted = new HashSet<>();
        var results = context.getSecurityProcessingResults(IEncryptionProcessingResult.class);
        if (results != null) {
            for (IEncryptionProcessingResult result : results) {
                if (result.isSuccessful() && AES.equals(result.getEncryptionAlgorithm())
                        && result.getEncryptedPayloads() != null) {
                    for (IPayloadEntity payload : result.getEncryptedPayloads()) {
                        encrypted.add(payload.getPayloadId());
                    }
                }
            }
        }
        boolean complete = message.getPayloads() != null && !message.getPayloads().isEmpty()
                && message.getPayloads().stream().allMatch(p -> encrypted.contains(p.getPayloadId()));
        if (!complete) {
            String detail = "All User Message payloads must be encrypted with AES-128-GCM";
            PolicyNoncompliance error = new PolicyNoncompliance(detail);
            error.setRefToMessageInError(message.getMessageId());
            context.addGeneratedError(error);
            HolodeckB2BCore.getStorageManager().setProcessingState(message, ProcessingState.FAILURE);
            log.warn("Encryption policy rejected message [{}]: {}", message.getMessageId(), detail);
        } else {
            log.info("Encryption policy verified AES-128-GCM for all {} payloads of message [{}]",
                    message.getPayloads().size(), message.getMessageId());
        }
        return InvocationResponse.CONTINUE;
    }
}
