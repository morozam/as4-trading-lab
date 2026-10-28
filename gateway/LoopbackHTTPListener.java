package lab.holodeck;

import java.io.IOException;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.ServerSocket;
import org.apache.axis2.AxisFault;
import org.apache.axis2.addressing.EndpointReference;
import org.apache.axis2.context.ConfigurationContext;
import org.apache.axis2.context.MessageContext;
import org.apache.axis2.context.SessionContext;
import org.apache.axis2.description.TransportInDescription;
import org.apache.axis2.kernel.TransportListener;
import org.apache.axis2.transport.http.HTTPTransportUtils;
import org.apache.axis2.transport.http.server.AxisHttpConnectionImpl;
import org.apache.axis2.transport.http.server.DefaultConnectionListener;
import org.apache.axis2.transport.http.server.DefaultConnectionListenerFailureHandler;
import org.apache.axis2.transport.http.server.HttpConnectionManager;
import org.apache.axis2.transport.http.server.IOProcessor;
import org.apache.axis2.transport.http.server.SimpleHttpServer;
import org.apache.http.params.HttpParams;
import org.holodeckb2b.core.axis2.HTTPFactory;

/** Local laboratory transport, pinned to Holodeck 8.1.1 / Axis2 1.8.2. */
public final class LoopbackHTTPListener implements TransportListener {
    private ConfigurationContext context;
    private TransportInDescription transport;
    private HTTPFactory factory;
    private SimpleHttpServer server;
    public void init(ConfigurationContext context, TransportInDescription transport) throws AxisFault {
        this.context = context;
        this.transport = transport;
        this.factory = new HTTPFactory(context, transport) {
            public IOProcessor newRequestConnectionListener(int port, HttpConnectionManager manager, HttpParams params) throws IOException {
                return new LoopbackConnections(port, manager, params);
            }
        };
    }
    public void start() throws AxisFault {
        try {
            server = new SimpleHttpServer(factory, factory.getPort());
            server.init();
            server.start();
        } catch (IOException e) { throw AxisFault.makeFault(e); }
    }
    public void stop() throws AxisFault {
        if (server != null) {
            try { server.destroy(); }
            catch (InterruptedException e) { Thread.currentThread().interrupt(); throw AxisFault.makeFault(e); }
            catch (IOException e) { throw AxisFault.makeFault(e); }
        }
    }
    public EndpointReference[] getEPRsForService(String service, String ip) throws AxisFault {
        if (server == null) throw new AxisFault("HTTP transport not started");
        return HTTPTransportUtils.getEPRsForService(context, transport, service, ip, server.getPort());
    }
    public SessionContext getSessionContext(MessageContext message) { return null; }
    public void destroy() { context = null; }

    /** Subclass required because SimpleHttpServer.start() casts its IOProcessor. */
    private static final class LoopbackConnections extends DefaultConnectionListener {
        private final ServerSocket socket;
        private final HttpConnectionManager manager;
        private final HttpParams params;
        private volatile boolean destroyed;
        LoopbackConnections(int port, HttpConnectionManager manager, HttpParams params) throws IOException {
            super(port, manager, new DefaultConnectionListenerFailureHandler(), params);
            this.manager = manager;
            this.params = params;
            socket = new ServerSocket();
            socket.setReuseAddress(true);
            try { socket.bind(new InetSocketAddress(InetAddress.getByName("127.0.0.1"), port)); }
            catch (IOException e) { socket.close(); throw e; }
        }
        public void run() {
            try {
                while (!destroyed && !Thread.currentThread().isInterrupted()) {
                    manager.process(new AxisHttpConnectionImpl(socket.accept(), params));
                }
            } catch (IOException e) {
                if (!destroyed) org.apache.logging.log4j.LogManager.getLogger(LoopbackHTTPListener.class).error("Loopback listener failed", e);
            } finally { destroy(); }
        }
        public synchronized void awaitSocketOpen() {}
        public synchronized int getPort() { return socket.getLocalPort(); }
        public synchronized boolean isDestroyed() { return destroyed; }
        public synchronized void close() throws IOException { destroyed = true; socket.close(); }
        public synchronized void destroy() { try { close(); } catch (IOException ignored) {} }
    }
}
